use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use bytes::BufMut;
// use io_uring::{opcode, squeue, types};
use libublk::io::{UblkDev, UblkIOCtx, UblkQueue};
use libublk::{ctrl::UblkCtrl, UblkError};
use log::{trace, info};
use serde::Serialize;

use aws_sdk_s3 as s3;

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
    async fn load(self) -> Result<S3Content, anyhow::Error> {
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

        let content = S3Content { size: u64::try_from(size)?, bytes: content };
        Ok(content)
    }
}

#[derive(Debug, Serialize)]
struct S3Content {
    size: u64,
    #[serde(skip_serializing)]
    bytes: bytes::BytesMut,
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
// fn lo_init_tgt(dev: &mut UblkDev, content_size: u64) -> Result<serde_json::Value, UblkError> {
//     trace!("s3: init_target {}", dev.dev_info.dev_id);
//     // if lo.direct_io != 0 {
//     //     unsafe {
//     //         libc::fcntl(lo.back_file.as_raw_fd(), libc::F_SETFL, libc::O_DIRECT);
//     //     }
//     // }

//     let dev_size = {
//         let tgt = &mut dev.tgt;
//         // let nr_fds = tgt.nr_fds;
//         // tgt.fds[nr_fds as usize] = s3_target.back_file.as_raw_fd();
//         // tgt.nr_fds = nr_fds + 1;

//         // tgt.dev_size = lo_file_size(&s3_target.back_file).unwrap();
//         tgt.dev_size = content_size;
//         tgt.dev_size
//     };
//     dev.set_default_params(dev_size);

//     Ok(
//         serde_json::json!({"content_size": content_size }),
//     )
// }

// fn loop_queue_tgt_io(
//     io: &mut UblkIOCtx,
//     tag: u32,
//     iod: &libublk::sys::ublksrv_io_desc,
// ) -> Result<i32, UblkError> {
//     let off = (iod.start_sector << 9) as u64;
//     let bytes = (iod.nr_sectors << 9) as u32;
//     let op = iod.op_flags & 0xff;
//     let data = UblkIOCtx::build_user_data(tag as u16, op, 0, true);
//     let buf_addr = io.io_buf_addr();
//     let r = io.get_ring();

//     if op == libublk::sys::UBLK_IO_OP_WRITE_ZEROES || op == libublk::sys::UBLK_IO_OP_DISCARD {
//         return Err(UblkError::OtherError(-libc::EINVAL));
//     }

//     match op {
//         libublk::sys::UBLK_IO_OP_FLUSH => {
//             let sqe = &opcode::SyncFileRange::new(types::Fixed(1), bytes)
//                 .offset(off)
//                 .build()
//                 .flags(squeue::Flags::FIXED_FILE)
//                 .user_data(data);
//             unsafe {
//                 r.submission().push(sqe).expect("submission fail");
//             }
//         }
//         libublk::sys::UBLK_IO_OP_READ => {
//             let sqe = &opcode::Read::new(types::Fixed(1), buf_addr, bytes)
//                 .offset(off)
//                 .build()
//                 .flags(squeue::Flags::FIXED_FILE)
//                 .user_data(data);
//             unsafe {
//                 r.submission().push(sqe).expect("submission fail");
//             }
//         }
//         libublk::sys::UBLK_IO_OP_WRITE => {
//             let sqe = &opcode::Write::new(types::Fixed(1), buf_addr, bytes)
//                 .offset(off)
//                 .build()
//                 .flags(squeue::Flags::FIXED_FILE)
//                 .user_data(data);
//             unsafe {
//                 r.submission().push(sqe).expect("submission fail");
//             }
//         }
//         _ => return Err(UblkError::OtherError(-libc::EINVAL)),
//     }

//     Ok(1)
// }

// fn loop_handle_io(ctx: &UblkQueueCtx, i: &mut UblkIOCtx) -> Result<i32, UblkError> {
//     let tag = i.get_tag();

//     // our IO on backing file is done
//     if i.is_tgt_io() {
//         let user_data = i.user_data();
//         let res = i.result();
//         let cqe_tag = UblkIOCtx::user_data_to_tag(user_data);

//         assert!(cqe_tag == tag);

//         if res != -(libc::EAGAIN) {
//             i.complete_io(res);

//             return Ok(0);
//         }
//     }

//     // either start to handle or retry
//     let _iod = ctx.get_iod(tag);
//     let iod = unsafe { &*_iod };

//     loop_queue_tgt_io(i, tag, iod)
// }

fn handle_io(
    io: &mut UblkIOCtx,
    iod: &libublk::sys::ublksrv_io_desc,
    content: Arc<Mutex<S3Content>>,
) -> Result<i32, UblkError> {
    let off = (iod.start_sector << 9) as u64;
    let bytes = (iod.nr_sectors << 9) as u32;
    let op = iod.op_flags & 0xff;
    let buf_addr = io.io_buf_addr();

    match op {
        libublk::sys::UBLK_IO_OP_READ => {
            let content  = content.lock().map_err(|_| UblkError::OtherError(-libc::EINVAL))?;
            let start = content.bytes.as_ptr() as u64;
            unsafe {
                libc::memcpy(
                    buf_addr as *mut libc::c_void,
                    (start + off) as *mut libc::c_void,
                    bytes as usize,
                );
            }
        },
        libublk::sys::UBLK_IO_OP_WRITE => {
            let mut content  = content.lock().map_err(|_| UblkError::OtherError(-libc::EINVAL))?;
            let start = content.bytes.as_mut_ptr() as u64;
            unsafe {
                libc::memcpy(
                    (start + off) as *mut libc::c_void,
                    buf_addr as *mut libc::c_void,
                    bytes as usize,
                );
            }
        },
        _ => return Err(UblkError::OtherError(-libc::EINVAL)),
    }

    io.complete_io(bytes as i32);
    Ok(0)
}

async fn test_add(dev_id: Option<i32>, bucket: String, object: String) -> Result<(), anyhow::Error> {
    let dev_id = dev_id.unwrap_or(0);
    let config = ::aws_config::load_from_env().await;
    let client = s3::Client::new(&config);

    // Target has to live in the whole device lifetime
    let s3_target = S3Target {
        bucket,
        object,
        client,
    };
    let content = s3_target.load().await?;

    let dev_size = content.size;
    let mut ctrl = create_ublk_ctrl(dev_id, true)?;
    let ublk_dev = UblkDev::new(
        "s3".to_string(),
        |dev: &mut UblkDev| {
            dev.set_default_params(dev_size);
            Ok(serde_json::json!({"content_size": dev_size}))
        },
        &mut ctrl,
        0,
    )?;

    let content = Arc::new(Mutex::new(content));

    let mut queue = UblkQueue::new(0, &ublk_dev).unwrap();
    let ctx = queue.make_queue_ctx();
    let qc = move |i: &mut UblkIOCtx| {
        let _iod = ctx.get_iod(i.get_tag());
        let iod = unsafe { &*_iod };

        handle_io(i, iod, content.clone())
    };
    ctrl.configure_queue(&ublk_dev, 0, unsafe { libc::gettid() });

    ctrl.start_dev_in_queue(&ublk_dev, &mut queue, &qc).unwrap();
    ctrl.dump();
    queue.wait_and_handle_io(&qc);

    ctrl.stop_dev(&ublk_dev).unwrap();
    
    Ok(())
}

fn test_del(dev_id: i32) {
    let mut ctrl = create_ublk_ctrl(dev_id, false).unwrap();

    ctrl.del().unwrap();
}

fn create_ublk_ctrl(dev_id: i32, for_add: bool) -> Result<UblkCtrl, UblkError> {
    UblkCtrl::new(dev_id, 1, 128,  512 << 10, 0, for_add)
}

#[::tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let dev_id = match std::env::args().nth(2) {
        Some(s) => Some(s.parse::<i32>()?),
        None => None,
    };

    if let Some(cmd) = std::env::args().nth(1) {
        match cmd.as_str() {
            "add" => {
                let bucket = std::env::args().nth(3).ok_or(anyhow!("missing bucket argument"))?;
                let object = std::env::args().nth(4).ok_or(anyhow!("missing object argument"))?;
                test_add(dev_id, bucket, object).await?
            },
            "del" => test_del(dev_id.unwrap_or(0)),
            _ => return Err(anyhow!("unknown command")),
        }
    }
    Err(anyhow!("missing command argument"))
}
