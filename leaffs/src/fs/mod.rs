use std::ffi::OsStr;
use std::path::Path;
use std::time::SystemTime;
use fuser::{Filesystem, KernelConfig, ReplyAttr, ReplyBmap, ReplyCreate, ReplyData, ReplyDirectory, ReplyDirectoryPlus, ReplyEmpty, ReplyEntry, ReplyIoctl, ReplyLock, ReplyLseek, ReplyOpen, ReplyStatfs, ReplyWrite, ReplyXattr, Request, TimeOrNow};
use libc::c_int;

pub struct FileSystem;

impl Filesystem for FileSystem {
    fn init(&mut self, _req: &Request<'_>, _config: &mut KernelConfig) -> Result<(), c_int> {
        todo!()
    }

    fn destroy(&mut self) {
        todo!()
    }

    fn lookup(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEntry) {
        todo!()
    }

    fn forget(&mut self, _req: &Request<'_>, _ino: u64, _nlookup: u64) {
        todo!()
    }

    fn getattr(&mut self, _req: &Request<'_>, ino: u64, reply: ReplyAttr) {
        todo!()
    }

    fn setattr(&mut self, _req: &Request<'_>, ino: u64, mode: Option<u32>, uid: Option<u32>, gid: Option<u32>, size: Option<u64>, _atime: Option<TimeOrNow>, _mtime: Option<TimeOrNow>, _ctime: Option<SystemTime>, fh: Option<u64>, _crtime: Option<SystemTime>, _chgtime: Option<SystemTime>, _bkuptime: Option<SystemTime>, flags: Option<u32>, reply: ReplyAttr) {
        todo!()
    }

    fn readlink(&mut self, _req: &Request<'_>, ino: u64, reply: ReplyData) {
        todo!()
    }

    fn mknod(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, mode: u32, umask: u32, rdev: u32, reply: ReplyEntry) {
        todo!()
    }

    fn mkdir(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, mode: u32, umask: u32, reply: ReplyEntry) {
        todo!()
    }

    fn unlink(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        todo!()
    }

    fn rmdir(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        todo!()
    }

    fn symlink(&mut self, _req: &Request<'_>, parent: u64, link_name: &OsStr, target: &Path, reply: ReplyEntry) {
        todo!()
    }

    fn rename(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, newparent: u64, newname: &OsStr, flags: u32, reply: ReplyEmpty) {
        todo!()
    }

    fn link(&mut self, _req: &Request<'_>, ino: u64, newparent: u64, newname: &OsStr, reply: ReplyEntry) {
        todo!()
    }

    fn open(&mut self, _req: &Request<'_>, _ino: u64, _flags: i32, reply: ReplyOpen) {
        todo!()
    }

    fn read(&mut self, _req: &Request<'_>, ino: u64, fh: u64, offset: i64, size: u32, flags: i32, lock_owner: Option<u64>, reply: ReplyData) {
        todo!()
    }

    fn write(&mut self, _req: &Request<'_>, ino: u64, fh: u64, offset: i64, data: &[u8], write_flags: u32, flags: i32, lock_owner: Option<u64>, reply: ReplyWrite) {
        todo!()
    }

    fn flush(&mut self, _req: &Request<'_>, ino: u64, fh: u64, lock_owner: u64, reply: ReplyEmpty) {
        todo!()
    }

    fn release(&mut self, _req: &Request<'_>, _ino: u64, _fh: u64, _flags: i32, _lock_owner: Option<u64>, _flush: bool, reply: ReplyEmpty) {
        todo!()
    }

    fn fsync(&mut self, _req: &Request<'_>, ino: u64, fh: u64, datasync: bool, reply: ReplyEmpty) {
        todo!()
    }

    fn opendir(&mut self, _req: &Request<'_>, _ino: u64, _flags: i32, reply: ReplyOpen) {
        todo!()
    }

    fn readdir(&mut self, _req: &Request<'_>, ino: u64, fh: u64, offset: i64, reply: ReplyDirectory) {
        todo!()
    }

    fn readdirplus(&mut self, _req: &Request<'_>, ino: u64, fh: u64, offset: i64, reply: ReplyDirectoryPlus) {
        todo!()
    }

    fn releasedir(&mut self, _req: &Request<'_>, _ino: u64, _fh: u64, _flags: i32, reply: ReplyEmpty) {
        todo!()
    }

    fn fsyncdir(&mut self, _req: &Request<'_>, ino: u64, fh: u64, datasync: bool, reply: ReplyEmpty) {
        todo!()
    }

    fn statfs(&mut self, _req: &Request<'_>, _ino: u64, reply: ReplyStatfs) {
        todo!()
    }

    fn getxattr(&mut self, _req: &Request<'_>, ino: u64, name: &OsStr, size: u32, reply: ReplyXattr) {
        todo!()
    }

    fn listxattr(&mut self, _req: &Request<'_>, ino: u64, size: u32, reply: ReplyXattr) {
        todo!()
    }

    fn removexattr(&mut self, _req: &Request<'_>, ino: u64, name: &OsStr, reply: ReplyEmpty) {
        todo!()
    }

    fn access(&mut self, _req: &Request<'_>, ino: u64, mask: i32, reply: ReplyEmpty) {
        todo!()
    }

    fn create(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, mode: u32, umask: u32, flags: i32, reply: ReplyCreate) {
        todo!()
    }

    fn getlk(&mut self, _req: &Request<'_>, ino: u64, fh: u64, lock_owner: u64, start: u64, end: u64, typ: i32, pid: u32, reply: ReplyLock) {
        todo!()
    }

    fn setlk(&mut self, _req: &Request<'_>, ino: u64, fh: u64, lock_owner: u64, start: u64, end: u64, typ: i32, pid: u32, sleep: bool, reply: ReplyEmpty) {
        todo!()
    }

    fn bmap(&mut self, _req: &Request<'_>, ino: u64, blocksize: u32, idx: u64, reply: ReplyBmap) {
        todo!()
    }

    fn ioctl(&mut self, _req: &Request<'_>, ino: u64, fh: u64, flags: u32, cmd: u32, in_data: &[u8], out_size: u32, reply: ReplyIoctl) {
        todo!()
    }

    fn fallocate(&mut self, _req: &Request<'_>, ino: u64, fh: u64, offset: i64, length: i64, mode: i32, reply: ReplyEmpty) {
        todo!()
    }

    fn lseek(&mut self, _req: &Request<'_>, ino: u64, fh: u64, offset: i64, whence: i32, reply: ReplyLseek) {
        todo!()
    }

    fn copy_file_range(&mut self, _req: &Request<'_>, ino_in: u64, fh_in: u64, offset_in: i64, ino_out: u64, fh_out: u64, offset_out: i64, len: u64, flags: u32, reply: ReplyWrite) {
        todo!()
    }
}