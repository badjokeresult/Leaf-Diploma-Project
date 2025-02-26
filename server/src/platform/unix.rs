use log::{info, warn};
use nix::sys::signal::{self, Signal};
use nix::sys::signalfd::{SigSet, SignalFd};
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use tokio::sync::oneshot;

// Функция для создания systemd unit файла
pub fn install_service(path: &str) -> Result<(), Box<dyn Error>> {
    let exec_path = std::env::current_exe()?;

    let service_content = format!(
        r#"[Unit]
Description=Leaf Server
After=network.target

[Service]
Type=simple
ExecStart={exec_path}
Restart=on-failure
User=leaf
Group=leaf
WorkingDirectory=/var/local/leaf

ProtectSystem=full
PrivateTmp=true
NoNewPrivileges=true

[Install]
WantedBy=multi-user.target
"#,
        exec_path = exec_path.display()
    );

    // Проверяем доступность директории
    let path_obj = Path::new(path);
    if let Some(parent) = path_obj.parent() {
        if !parent.exists() {
            return Err(format!("Директория {} не существует", parent.display()).into());
        }
    }

    // Записываем unit файл
    let mut file = File::create(path)?;
    file.write_all(service_content.as_bytes())?;

    Ok(())
}

pub fn uninstall_service() -> Result<(), Box<dyn Error>> {
    let service_path = "/etc/systemd/system/leaf-server.service";

    if Path::new(service_path).exists() {
        // Пытаемся остановить службу, если она запущена
        let _ = std::process::Command::new("systemctl")
            .args(["stop", "leaf-server"])
            .status();

        // Удаляем unit файл
        std::fs::remove_file(service_path)?;

        info!("Systemd unit файл успешно удален");
        info!("Выполните: sudo systemctl daemon-reload для применения изменений");
    } else {
        info!("Systemd unit файл не найден");
    }

    Ok(())
}

pub fn start_service() -> Result<(), Box<dyn Error>> {
    let status = std::process::Command::new("systemctl")
        .args(["start", "leaf-server"])
        .status()?;

    if status.success() {
        info!("Служба успешно запущена");
    } else {
        return Err("Не удалось запустить службу через systemctl".into());
    }

    Ok(())
}

pub fn stop_service() -> Result<(), Box<dyn Error>> {
    let status = std::process::Command::new("systemctl")
        .args(["stop", "leaf-server"])
        .status()?;

    if status.success() {
        info!("Служба успешно остановлена");
    } else {
        return Err("Не удалось остановить службу через systemctl".into());
    }

    Ok(())
}

#[allow(dead_code)]
pub fn run_as_service() -> Result<(), Box<dyn Error>> {
    Ok(())
}

// Настройка обработчика сигналов для корректного завершения
pub fn setup_signal_handler(shutdown_sender: oneshot::Sender<()>) -> Result<(), Box<dyn Error>> {
    // Создаем обработчик в отдельном потоке
    std::thread::spawn(move || {
        // Набор сигналов, которые мы хотим обрабатывать
        let mut mask = SigSet::empty();
        mask.add(Signal::SIGINT);
        mask.add(Signal::SIGTERM);

        // Блокируем эти сигналы в текущем потоке
        if let Err(e) = signal::pthread_sigmask(signal::SigmaskHow::SIG_BLOCK, Some(&mask), None) {
            warn!("Не удалось заблокировать сигналы: {}", e);
            return;
        }

        // Создаем файловый дескриптор сигналов
        let mut signal_fd = match SignalFd::new(&mask) {
            Ok(fd) => fd,
            Err(e) => {
                warn!("Не удалось создать signalfd: {}", e);
                return;
            }
        };

        // Ожидаем сигнал
        match signal_fd.read_signal() {
            Ok(Some(info)) => {
                info!("Получен сигнал: {}", info.ssi_signo);
                // Отправляем сигнал завершения основному потоку
                let _ = shutdown_sender.send(());
            }
            Ok(None) => {
                warn!("Нет данных в signalfd");
            }
            Err(e) => {
                warn!("Ошибка чтения из signalfd: {}", e);
            }
        }
    });

    Ok(())
}
