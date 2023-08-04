use anyhow::Result;
use bytes::BufMut;
use io_uring::{opcode, squeue, types};
use libublk::io::{UblkDev, UblkIOCtx, UblkQueueCtx};
use libublk::{ctrl::UblkCtrl, UblkError};
use log::{trace, info};
use serde::Serialize;

// #[::thiserror::Error]
// pub enum TargetError {
//     S3Error {
//         #[from]
//         err: s3::error::SdkError<>
//     }
// }

#[derive(Debug, Serialize)]
struct S3Target {
    bucket: String,
    object: String,
    #[serde(skip_serializing)]
    client: s3::Client,
}

impl S3Target {
    async fn load(self) -> Result<(S3DevInitParams, Content), anyhow::Error> {
        let obj_attrs = self.client
            .get_object_attributes()
            .bucket(&self.bucket)
            .key(&self.object)
            .send()
            .await?;
        let size = u64::try_from(obj_attrs.object_size())?;
        info!("Found object {} / {} to be of size: {} bytes", self.bucket, self.object, size);

        let mut object = self.client
            .get_object()
            .bucket(&self.bucket)
            .key(&self.object)
            .send()
            .await?;
        let mut content = bytes::BytesMut::with_capacity(size.try_into()?);
        
        use tokio_stream::StreamExt;
        let mut _byte_count = 0_usize;
        while let Some(bytes) = object.body.try_next().await? {
            let bytes_read = bytes.len();
            content.put(bytes);
            _byte_count += bytes_read;
            trace!("Intermediate write of {}", bytes_read);
        }

        let params = S3DevInitParams { dev_size: u64::try_from(size)? };
        let content = Content { bytes: content };
        Ok((params, content))
    }
}

struct Content {
    bytes: bytes::BytesMut,
}

#[derive(Debug, Serialize)]
struct S3DevInitParams {
    dev_size: u64,
}

// fn lo_file_size(f: &std::fs::File) -> Result<u64> {
//     if let Ok(meta) = f.metadata() {
//         if meta.file_type().is_file() {
//             Ok(f.metadata().unwrap().len())
//         } else {
//             Err(anyhow::anyhow!("unsupported file"))
//         }
//     } else {
//         Err(anyhow::anyhow!("no file meta got"))
//     }
// }

// setup s3 target
fn lo_init_tgt(dev: &mut UblkDev, params: &S3DevInitParams) -> Result<serde_json::Value, UblkError> {
    trace!("s3: init_target {}", dev.dev_info.dev_id);
    // if lo.direct_io != 0 {
    //     unsafe {
    //         libc::fcntl(lo.back_file.as_raw_fd(), libc::F_SETFL, libc::O_DIRECT);
    //     }
    // }

    let dev_size = {
        let tgt = &mut dev.tgt;
        // let nr_fds = tgt.nr_fds;
        // tgt.fds[nr_fds as usize] = s3_target.back_file.as_raw_fd();
        // tgt.nr_fds = nr_fds + 1;

        // tgt.dev_size = lo_file_size(&s3_target.back_file).unwrap();
        tgt.dev_size = params.dev_size;
        tgt.dev_size
    };
    dev.set_default_params(dev_size);

    Ok(
        serde_json::json!({"S3DevInitParams": params }),
    )
}

fn loop_queue_tgt_io(
    io: &mut UblkIOCtx,
    tag: u32,
    iod: &libublk::sys::ublksrv_io_desc,
) -> Result<i32, UblkError> {
    let off = (iod.start_sector << 9) as u64;
    let bytes = (iod.nr_sectors << 9) as u32;
    let op = iod.op_flags & 0xff;
    let data = UblkIOCtx::build_user_data(tag as u16, op, 0, true);
    let buf_addr = io.io_buf_addr();
    let r = io.get_ring();

    if op == libublk::sys::UBLK_IO_OP_WRITE_ZEROES || op == libublk::sys::UBLK_IO_OP_DISCARD {
        return Err(UblkError::OtherError(-libc::EINVAL));
    }

    match op {
        libublk::sys::UBLK_IO_OP_FLUSH => {
            let sqe = &opcode::SyncFileRange::new(types::Fixed(1), bytes)
                .offset(off)
                .build()
                .flags(squeue::Flags::FIXED_FILE)
                .user_data(data);
            unsafe {
                r.submission().push(sqe).expect("submission fail");
            }
        }
        libublk::sys::UBLK_IO_OP_READ => {
            let sqe = &opcode::Read::new(types::Fixed(1), buf_addr, bytes)
                .offset(off)
                .build()
                .flags(squeue::Flags::FIXED_FILE)
                .user_data(data);
            unsafe {
                r.submission().push(sqe).expect("submission fail");
            }
        }
        libublk::sys::UBLK_IO_OP_WRITE => {
            let sqe = &opcode::Write::new(types::Fixed(1), buf_addr, bytes)
                .offset(off)
                .build()
                .flags(squeue::Flags::FIXED_FILE)
                .user_data(data);
            unsafe {
                r.submission().push(sqe).expect("submission fail");
            }
        }
        _ => return Err(UblkError::OtherError(-libc::EINVAL)),
    }

    Ok(1)
}

fn loop_handle_io(ctx: &UblkQueueCtx, i: &mut UblkIOCtx) -> Result<i32, UblkError> {
    let tag = i.get_tag();

    // our IO on backing file is done
    if i.is_tgt_io() {
        let user_data = i.user_data();
        let res = i.result();
        let cqe_tag = UblkIOCtx::user_data_to_tag(user_data);

        assert!(cqe_tag == tag);

        if res != -(libc::EAGAIN) {
            i.complete_io(res);

            return Ok(0);
        }
    }

    // either start to handle or retry
    let _iod = ctx.get_iod(tag);
    let iod = unsafe { &*_iod };

    loop_queue_tgt_io(i, tag, iod)
}

async fn test_add() -> Result<(), anyhow::Error> {
    let config = ::aws_config::load_from_env().await;
    let client = s3::Client::new(&config);

    let bucket = std::env::args().nth(2).unwrap();
    let object = std::env::args().nth(3).unwrap();

    let _pid = unsafe { libc::fork() };

    if _pid == 0 {
        // Target has to live in the whole device lifetime
        let s3_target = S3Target {
            bucket,
            object,
            client,
        };
        let (s3_params, content) = s3_target.load().await?;
        libublk::ublk_tgt_worker(
            "s3".to_string(),
            -1,
            1,
            64,
            512_u32 * 1024,
            0,
            true,
            0,
            |dev: &mut UblkDev| lo_init_tgt(dev, &s3_params),
            loop_handle_io,
            |dev_id| {
                let mut ctrl = UblkCtrl::new(dev_id, 0, 0, 0, 0, false).unwrap();

                ctrl.dump();
            },
        )
        .unwrap()
        .join().map_err(|err| anyhow::anyhow!("err: {:#?}", err) )?;
    }
    Ok(())
}

fn test_del() {
    let s = std::env::args().nth(2).unwrap_or_else(|| "0".to_string());
    let dev_id = s.parse::<i32>().unwrap();
    let mut ctrl = UblkCtrl::new(dev_id as i32, 0, 0, 0, 0, false).unwrap();

    ctrl.del().unwrap();
}

use aws_sdk_s3 as s3;

#[::tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    if let Some(cmd) = std::env::args().nth(1) {
        match cmd.as_str() {
            "add" => test_add().await?,
            "del" => test_del(),
            _ => todo!(),
        }
    }

    Ok(())
}
