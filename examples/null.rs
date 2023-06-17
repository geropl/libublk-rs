use anyhow::Result;
use libublk::{UblkCtrl, UblkDev, UblkIO, UblkQueue, UblkQueueImpl};
use std::sync::Arc;

pub struct NullTgt {}
pub struct NullQueue {}

// setup null target
impl libublk::UblkTgtImpl for NullTgt {
    fn init_tgt(&self, dev: &UblkDev) -> Result<serde_json::Value> {
        let info = dev.dev_info;
        let dev_size = 250_u64 << 30;

        let mut tgt = dev.tgt.borrow_mut();

        tgt.dev_size = dev_size;
        tgt.params = libublk::ublk_params {
            types: libublk::UBLK_PARAM_TYPE_BASIC,
            basic: libublk::ublk_param_basic {
                logical_bs_shift: 9,
                physical_bs_shift: 12,
                io_opt_shift: 12,
                io_min_shift: 9,
                max_sectors: info.max_io_buf_bytes >> 9,
                dev_sectors: dev_size >> 9,
                ..Default::default()
            },
            ..Default::default()
        };

        Ok(serde_json::json!({}))
    }
    fn deinit_tgt(&self, _dev: &UblkDev) {}
}

// implement io logic, and it is the main job for writing new ublk target
impl libublk::UblkQueueImpl for NullQueue {
    fn queue_io(&self, q: &UblkQueue, io: &mut UblkIO, tag: u32) -> Result<i32> {
        let iod = q.get_iod(tag);
        let bytes = unsafe { (*iod).nr_sectors << 9 } as i32;

        q.complete_io(io, tag as u16, bytes);
        Ok(0)
    }
    fn tgt_io_done(&self, _q: &UblkQueue, _io: &mut UblkIO, _tag: u32, _res: i32, _user_data: u64) {
    }
}

fn test_add() {
    let s = std::env::args().nth(2).unwrap_or_else(|| "-1".to_string());
    let dev_id = s.parse::<i32>().unwrap();
    let _pid = unsafe { libc::fork() };
    if _pid == 0 {
        libublk::ublk_tgt_worker(
            dev_id,
            2,
            64,
            512_u32 * 1024,
            0,
            "null".to_string(),
            || Box::new(NullTgt {}),
            Arc::new(|| Box::new(NullQueue {}) as Box<dyn UblkQueueImpl>),
            |dev_id| {
                let mut ctrl = UblkCtrl::new(dev_id, 0, 0, 0, 0, false).unwrap();

                ctrl.dump();
            },
        )
        .unwrap()
        .join()
        .unwrap();
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
