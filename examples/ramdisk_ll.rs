use core::any::Any;
use libublk::io::{UblkDev, UblkQueue, UblkQueueImpl, UblkTgtImpl};
use libublk::{ctrl::UblkCtrl, UblkError};
use std::sync::Arc;

struct RamdiskTgt {
    size: u64,
    start: u64,
}

struct RamdiskQueue {}

// setup ramdisk target
impl UblkTgtImpl for RamdiskTgt {
    fn init_tgt(&self, dev: &UblkDev) -> Result<serde_json::Value, UblkError> {
        let info = dev.dev_info;
        let dev_size = self.size;

        let mut tgt = dev.tgt.borrow_mut();

        tgt.dev_size = dev_size;
        tgt.params = libublk::sys::ublk_params {
            types: libublk::sys::UBLK_PARAM_TYPE_BASIC,
            basic: libublk::sys::ublk_param_basic {
                logical_bs_shift: 12,
                physical_bs_shift: 12,
                io_opt_shift: 12,
                io_min_shift: 12,
                max_sectors: info.max_io_buf_bytes >> 9,
                dev_sectors: dev_size >> 9,
                ..Default::default()
            },
            ..Default::default()
        };

        Ok(serde_json::json!({}))
    }
    fn tgt_type(&self) -> &'static str {
        "ramdisk"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// implement io logic, and it is the main job for writing new ublk target
impl UblkQueueImpl for RamdiskQueue {
    fn handle_io_cmd(&self, q: &mut UblkQueue, tag: u32) -> Result<i32, UblkError> {
        let _iod = q.get_iod(tag);
        let iod = unsafe { &*_iod };
        let off = (iod.start_sector << 9) as u64;
        let bytes = (iod.nr_sectors << 9) as u32;
        let op = iod.op_flags & 0xff;
        let tgt = q.dev.ublk_tgt_data_from_queue::<RamdiskTgt>().unwrap();
        let start = tgt.start;
        let buf_addr = q.get_buf_addr(tag);

        match op {
            libublk::sys::UBLK_IO_OP_READ => unsafe {
                libc::memcpy(
                    buf_addr as *mut libc::c_void,
                    (start + off) as *mut libc::c_void,
                    bytes as usize,
                );
            },
            libublk::sys::UBLK_IO_OP_WRITE => unsafe {
                libc::memcpy(
                    (start + off) as *mut libc::c_void,
                    buf_addr as *mut libc::c_void,
                    bytes as usize,
                );
            },
            _ => return Err(UblkError::OtherError(-libc::EINVAL)),
        }

        q.complete_io(tag as u16, bytes as i32);
        Ok(0)
    }
}

fn rd_add_dev(dev_id: i32, buf_addr: u64, size: u64) {
    let depth = 128;
    let nr_queues = 1;
    let mut ctrl = UblkCtrl::new(dev_id, nr_queues, depth, 512 << 10, 0, true).unwrap();
    let ublk_dev = Arc::new(
        UblkDev::new(
            Box::new(RamdiskTgt {
                size,
                start: buf_addr,
            }),
            &mut ctrl,
        )
        .unwrap(),
    );

    let mut affinity = libublk::ctrl::UblkQueueAffinity::new();
    ctrl.get_queue_affinity(0, &mut affinity).unwrap();
    let _dev = Arc::clone(&ublk_dev);
    let _qid = 0;

    let qh = std::thread::spawn(move || {
        unsafe {
            libc::pthread_setaffinity_np(
                libc::pthread_self(),
                affinity.buf_len(),
                affinity.addr() as *const libc::cpu_set_t,
            );
        }
        let ops = RamdiskQueue {};

        UblkQueue::new(_qid, &_dev, depth, depth, 0)
            .unwrap()
            .handler(&ops);
    });

    ctrl.start_dev(&ublk_dev).unwrap();
    ctrl.dump();

    qh.join()
        .unwrap_or_else(|_| eprintln!("dev-{} join queue thread failed", ublk_dev.dev_info.dev_id));

    ctrl.stop_dev(&ublk_dev).unwrap();
}

fn test_add() {
    let dev_id: i32 = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "-1".to_string())
        .parse::<i32>()
        .unwrap();
    let s = std::env::args().nth(3).unwrap_or_else(|| "32".to_string());
    let mb = s.parse::<u64>().unwrap();

    let _pid = unsafe { libc::fork() };
    if _pid == 0 {
        let size = (mb << 20) as u64;
        let buf = libublk::ublk_alloc_buf(size as usize, 4096);

        rd_add_dev(dev_id, buf as u64, size);

        libublk::ublk_dealloc_buf(buf, size as usize, 4096);
    }
}

fn test_del() {
    let s = std::env::args().nth(2).unwrap_or_else(|| "0".to_string());
    let dev_id = s.parse::<i32>().unwrap();
    let mut ctrl = UblkCtrl::new(dev_id as i32, 0, 0, 0, 0, false).unwrap();

    ctrl.del().unwrap();
}

fn main() {
    if let Some(cmd) = std::env::args().nth(1) {
        match cmd.as_str() {
            "add" => test_add(),
            "del" => test_del(),
            _ => todo!(),
        }
    }
}