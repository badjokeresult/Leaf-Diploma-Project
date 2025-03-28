use std::path::{Path, PathBuf}; // Зависимость стандартной библиотеки для работы с файловыми путями

use clap::Parser; // Внешние зависимости для работы с аргументами командной строки
use clap::{arg, command};
use clap_derive::{Parser, ValueEnum};

use leafcommon;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    // Структура для хранения аргументов командной строки
    #[arg(value_enum, short, long)]
    action: Action, // Аргумент, отвечающий за реализуемое действие
    #[arg(short, long)]
    file: String, // Аргумент, указывающий целевой файл
}

impl Args {
    pub fn get_action(&self) -> Action {
        // Получение аргумента действия
        self.action
    }
    pub fn get_file(&self) -> PathBuf {
        // Получение аргумента пути к файлу
        PathBuf::from(&self.file)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, ValueEnum)]
pub enum Action {
    // Перечисление, хранящее возможные варианты действий
    Send,    // Действие по отправке файла
    Receive, // Действие по получению файла
}

pub fn load_args() -> Args {
    // Функция парсинга полученных аргументов
    Args::parse()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = load_args(); // Получение аргументов командной строки

    // Используем тот же пароль для шифрования шифровальщика
    let path = &args.file;
    match args.get_action() {
        Action::Send => send_file(path).await,
        Action::Receive => recv_file(path).await, // Если получение - вызываем функцию получения
    }
}

async fn send_file(path: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
    leafcommon::reed_solomon_scheme::send_file(path).await
}

async fn recv_file(path: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
    leafcommon::reed_solomon_scheme::recv_file(path).await
}
