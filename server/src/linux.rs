use crate::server::AsyncServer;
use daemonize::Daemonize;
use log::{error, info};
use std::{fs::File, sync::atomic::Ordering::SeqCst};
use tokio::signal::unix::{signal, SignalKind};

pub async fn run_service() {
    let stdout = File::create("/var/local/leaf/leaf.out").unwrap();
    let stderr = File::create("/var/local/leaf/leaf.err").unwrap();

    let daemonize = Daemonize::new()
        .pid_file("/var/local/leaf/leaf.pid")
        .chown_pid_file(true)
        .working_directory("/var/local/leaf")
        .user("leaf-server")
        .group("leaf-server")
        .umask(0o777)
        .stdout(stdout)
        .stderr(stderr);

    match daemonize.start() {
        Ok(_) => {
            service_main_loop().await;
        }
        Err(e) => error!("Ошибка запуска демона: {}", e),
    }
}

async fn service_main_loop() {
    // Создаем экземпляр сервера
    let server = AsyncServer::new();
    let shutdown_signal = server.get_shutdown_signal();

    let server_handle = tokio::spawn(async move {
        server.run().await;
    });

    // Настраиваем обработку сигналов SIGTERM и SIGINT
    let mut sigterm =
        signal(SignalKind::terminate()).expect("Не удалось настроить обработку SIGTERM");
    let mut sigint =
        signal(SignalKind::interrupt()).expect("Не удалось настроить обработку SIGINT");

    // Запускаем сервер и обработку сигналов в отдельных тасках
    tokio::select! {
        _ = sigterm.recv() => {
            info!("Получен сигнал SIGTERM");
            shutdown_signal.store(true, SeqCst);
        }
        _ = sigint.recv() => {
            info!("Получен сигнал SIGINT");
            shutdown_signal.store(true, SeqCst);
        }
    }

    if let Err(e) = server_handle.await {
        error!("Error: {}", e);
    }

    info!("Linux-служба завершена");
}
