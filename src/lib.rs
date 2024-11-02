use std::net::SocketAddr;
use std::ffi::CStr;
use std::os::raw::{c_char, c_ulong, c_void, c_ushort};
use std::sync::Mutex;

use tokio::sync::mpsc::{channel, Receiver};
use tokio::runtime::{Builder, Runtime};
use tokio::task::{spawn, JoinHandle};

use ctor::*;
use libc_print::libc_println;

mod codec;
mod crypto;
mod hash;
mod message;
mod server;
mod shared_secret;
mod storage;

use message::*;
use server::*;
use shared_secret::*;
use crypto::*;
use hash::*;

pub mod consts {
    pub const WORKING_FOLDER_NAME: &str = ".leaf";
    pub const PASSWORD_FILE_NAME: &str = "passwd.txt";
    pub const GAMMA_FILE_NAME: &str = "gamma.bin";
    pub const SENDING_REQUEST_TYPE: u8 = 0;
    pub const SENDING_ACKNOWLEDGEMENT_TYPE: u8 = 1;
    pub const RETRIEVING_REQUEST_TYPE: u8 = 2;
    pub const RETRIEVING_ACKNOWLEDGEMENT_TYPE: u8 = 3;
    pub const CONTENT_FILLED_TYPE: u8 = 4;
    pub const EMPTY_TYPE: u8 = 5;
    pub const MAX_MESSAGE_SIZE: usize = 65243;
    pub const MAX_DATAGRAM_SIZE: usize = 65507;
    pub const DEFAULT_CHUNKS_STOR_FOLDER: &str = "chunks";
}

const LOCAL_ADDR_STR: *const c_char = "0.0.0.0:62092".as_ptr() as *const c_char;
const BROADCAST_ADDR_STR: *const c_char = "255.255.255.255:62092".as_ptr() as *const c_char;

#[repr(C)]
pub struct CVec {
    len: c_ulong,
    capacity: c_ulong,
    data: *mut c_void,
}

pub struct InitializeParams {
    pub receiver: Receiver<(Message, SocketAddr)>,
    pub server: BroadcastUdpServer,
    pub tokio_builder: Runtime,
    pub handles_vec: Vec<JoinHandle<()>>,
}

#[ctor]
pub static INIT_PARAMS: Mutex<InitializeParams> = {
    let builder = Builder::new_multi_thread()
        .enable_all()
        .build().unwrap();

    let addr = unsafe { CStr::from_ptr(LOCAL_ADDR_STR).to_str().unwrap() };
    let broadcast_addr = unsafe { CStr::from_ptr(BROADCAST_ADDR_STR).to_str().unwrap() };
    let (tx, rx) = channel::<(Message, SocketAddr)>(1024);
    let server = builder.block_on(async { BroadcastUdpServer::new(addr, broadcast_addr, tx.clone()).await });

    let num_threads = num_cpus::get();
    let mut handles = vec![];
    for _ in 0..num_threads {
        let server = server.clone();
        handles.push(builder.block_on(async { spawn(async move {
            server.listen().await;
        }) }));
    };

    let result = Mutex::new(InitializeParams {
        receiver: rx,
        server,
        tokio_builder: builder,
        handles_vec: handles,
    });

    print_log("INIT DONE");

    result
};

#[cfg(not(windows))]
fn print_log(message: &str) {
    libc_println!("{}", message);
}

#[cfg(windows)]
fn print_log(message: &str) {
    let mut con = winapi_util::console::Console::stdout().unwrap();
    println!("{}", message);
    con.reset().unwrap();
}

#[no_mangle]
pub extern "C" fn send_file(len: c_ulong, capacity: c_ulong, content: *mut c_ushort) -> CVec {
    let content: Vec<u8> = unsafe { Vec::from_raw_parts(content as *mut u8, len as usize, capacity as usize) };

    let sharer = ReedSolomonSecretSharer::new();
    let chunks = sharer.split_into_chunks(&content).unwrap();

    let encryptor = KuznechikEncryptor::new().unwrap();
    let mut encrypted_chunks = vec![];
    for chunk in chunks {
        if let Some(c) = chunk {
            encrypted_chunks.push(Some(INIT_PARAMS.lock().unwrap().tokio_builder.block_on(async { encryptor.encrypt_chunk(&c).await.unwrap() })));
        } else {
            encrypted_chunks.push(None);
        }
    }

    let hasher = StreebogHasher::new();
    let mut hashes = vec![];
    for chunk in &encrypted_chunks {
        if let Some(c) = chunk {
            hashes.push(Some(hasher.calc_hash_for_chunk(c)));
        } else {
            hashes.push(None);
        }
    }

    let mid = encrypted_chunks.len() / 2;

    for i in 0..mid {
        let (chunk, hash) = match encrypted_chunks.get(i).unwrap() {
            Some(c) => match hashes.get(i).unwrap() {
                Some(h) => (c, h),
                None => panic!(),
            },
            None => match encrypted_chunks.get(mid + i).unwrap() {
                Some(c) => match hashes.get(mid + i).unwrap() {
                    Some(h) => (c, h),
                    None => panic!(),
                },
                None => panic!(),
            }
        };
        INIT_PARAMS.lock().unwrap().tokio_builder.block_on(async { INIT_PARAMS.lock().unwrap().server.send_chunk(hash, chunk, &mut INIT_PARAMS.lock().unwrap().receiver).await }).unwrap();
    }

    let len = hashes.len();
    let capacity = hashes.capacity();
    let hashes_ptr = Box::into_raw(Vec::into_boxed_slice(hashes)) as *mut c_void;

    CVec {
        len: len as c_ulong,
        capacity: capacity as c_ulong,
        data: hashes_ptr,
    }
}

#[no_mangle]
pub extern "C" fn recv_content(len: c_ushort, capacity: c_ushort, hashes: *mut c_void) -> CVec {
    let hashes: Vec<Option<Vec<u8>>> = unsafe { Vec::from_raw_parts(hashes as *mut Option<Vec<u8>>, len as usize, capacity as usize) };

    let mut chunks = vec![];
    for hash in hashes {
        if let Some(c) = hash {
            chunks.push(Some(INIT_PARAMS.lock().unwrap().tokio_builder.block_on(async { INIT_PARAMS.lock().unwrap().server.recv_chunk(&c, &mut INIT_PARAMS.lock().unwrap().receiver).await.unwrap() })));
        } else {
            chunks.push(None);
        }
    }

    let decryptor = KuznechikEncryptor::new().unwrap();
    let mut decrypted_chunks = vec![];
    for chunk in chunks {
        if let Some(c) = chunk {
            decrypted_chunks.push(Some(INIT_PARAMS.lock().unwrap().tokio_builder.block_on(async { decryptor.decrypt_chunk(&c).await.unwrap() })));
        } else {
            decrypted_chunks.push(None);
        }
    }

    let sharer = ReedSolomonSecretSharer::new();
    let content = sharer.recover_from_chunks(decrypted_chunks).unwrap();

    let len = content.len();
    let capacity = content.capacity();
    let content_ptr = Box::into_raw(Vec::into_boxed_slice(content)) as *mut c_void;

    CVec {
        len: len as c_ulong,
        capacity: capacity as c_ulong,
        data: content_ptr
    }
}

#[dtor]
fn shutdown() {
    let _: Vec<_> = INIT_PARAMS.lock().unwrap().handles_vec.iter().map(|x| x.abort()).collect();

    let server = INIT_PARAMS.lock().unwrap().server.clone();
    INIT_PARAMS.lock().unwrap().tokio_builder.block_on(async { server.shutdown().await; });

    INIT_PARAMS.lock().unwrap().receiver.close();
}