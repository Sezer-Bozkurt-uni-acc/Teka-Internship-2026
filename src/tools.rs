use std::{collections::HashMap, env, fs::{self, File}, io::{Error, ErrorKind, Write}, path::Path};
use chrono::{Local};

use crate::{client::ClientConfig, server::ServerConfig, tools};

const EXIT_CODE_SUCCESS: i32 = 0;
const EXIT_CODE_FILE_NOT_FOUND: i32 = 3;
const EXIT_CODE_IO_ERROR: i32 = 4;

const INFO_PREFIX:     &str = "INFO";
const WARN_PREFIX:     &str = "WARN";
const ERROR_PREFIX:    &str = "ERROR";

const CLIENT_NAME: &str = "client";
const SERVER_NAME: &str = "server";


pub fn get_config(client: bool) -> Box<dyn Config>
{
    if client { Box::new(ClientConfig::get_or_throw()) }
    else      { Box::new(ServerConfig::get_or_throw()) }
}

pub fn get_valid_config() -> Box<dyn Config>
{
    match ClientConfig::get() 
    {
        Some(client_config) => { Box::new(client_config) }    
        None =>                              { Box::new(ServerConfig::get_or_throw()) }
    }
}

pub fn create_string_map(file_name: &str, section: &str) -> std::io::Result<HashMap<String, String>>
{
    match env::current_exe()
    {
        Ok(exe_path) =>
        {
            let directory = exe_path.parent().expect("Error locating parent directory");
            let path = directory.join(file_name);
            let contents = fs::read_to_string(path)?;

            let wrapped: String = format!("[{}]", section);
            let mut map: HashMap<String, String> = HashMap::new();
            let mut found = false;
            let mut line_empty: bool;
            let mut line_section: bool;
            
            for line in contents.lines()
            {
                line_empty = line.is_empty();
                line_section = line.starts_with("[");

                if found
                {
                    if line_empty || line_section { break; }
                    else { tools::fill_string_map(&mut map, line); }
                }
                else if line_section
                {
                    found = line == &wrapped;
                }
            }
            
            if !found              { Err(Error::new(ErrorKind::NotFound, "Section not found".to_string())) }
            else if map.is_empty() { Err(Error::new(ErrorKind::NotFound, "Data not found".to_string())) }
            else                   { Ok(map) }
        }
        Err(error) => { Err(error) }
    }
}


fn fill_string_map(map: &mut HashMap<String, String>, line: &str)
{
    let mut parts = line.splitn(2, "=");
    let key = parts.next().unwrap_or("").trim().to_string();
    let value = parts.next().unwrap_or("").trim().trim_matches('"').to_string();

    if !key.is_empty() && !value.is_empty() { map.insert(key, value); }
}

pub fn clear_directory(directory_path: &str) -> std::io::Result<i32>
{
    let path = Path::new(directory_path);
    
    if !path.exists()      { return Ok(EXIT_CODE_FILE_NOT_FOUND); }
    else if !path.is_dir() { return Ok(EXIT_CODE_IO_ERROR); }

    let entries = fs::read_dir(path)?;

    for entry in entries 
    {
        let entry = entry?;
        let entry_path = entry.path();

        if entry_path.is_dir() { fs::remove_dir_all(&entry_path)?; }
        else                   { fs::remove_file(&entry_path)?; }
    }

    Ok(EXIT_CODE_SUCCESS)
}


#[derive(Debug)]
pub struct Logger 
{
    folder_path: String,
    file_path: String,
    branch: String,
    has_file: bool
}

impl Logger
{
    pub fn create_main() -> Logger
    {
        Logger 
        { 
            folder_path: String::new(), 
            file_path: String::new(), 
            branch: String::from("MAIN"), 
            has_file: false
        }
    }

    pub fn create(branch: &str, folder_path: &str) -> Logger 
    {
        let date_string: String = Local::now().date_naive().to_string();
        let mut index: u32 = 0;
        let mut search: bool = true;
        let mut file_path: String = String::new();

        while search
        {
            index += 1;
            file_path = format!("{}/{}-log-{}.txt", &folder_path, &date_string, &index);
            search = Path::new(&file_path).exists();
        }

        Logger { folder_path: folder_path.to_string(), file_path: file_path, branch: branch.to_string(), has_file: true }
    }
    
    pub fn branch(&self, new_branch: &str) -> Logger 
    {
        Logger 
        {
            folder_path: self.folder_path.clone(), 
            file_path: self.file_path.clone(), 
            branch: new_branch.to_string(),
            has_file: self.has_file
        }
    }

    fn get_file(&self) -> File
    {
        fs::OpenOptions::new().create(true).append(true).open(&self.file_path).unwrap()
    }

    fn log(&self, prefix: &str, message: String)
    {
        let date = Local::now().format("%H:%M:%S");
        let full_message: String = format!("[{} / {}] [{}] {}\n", date, self.branch, prefix, message);

        print!("{}", &full_message);

        if self.has_file
        {
            let mut file = self.get_file();
            let _ = file.write_all(full_message.as_bytes());
        }
    }

    pub fn info(&self, message: &str) { self.info_literal(message.to_string()); }

    pub fn warn(&self, message: &str) { self.warn_literal(message.to_string()); }

    pub fn error(&self, message: &str) { self.error_literal(message.to_string()) }

    pub fn info_literal(&self, message: String) { self.log(INFO_PREFIX, message); }

    pub fn warn_literal(&self, message: String) { self.log(WARN_PREFIX, message); }
    
    pub fn error_literal(&self, message: String) { self.log(ERROR_PREFIX, message); }

}

impl Clone for Logger
{
    fn clone(&self) -> Self 
    {
        Logger 
        {
            folder_path: self.folder_path.clone(), 
            file_path: self.file_path.clone(), 
            branch: self.branch.clone(),
            has_file: self.has_file
        }
    }
}

pub trait Sided
{
    fn is_client(&self) -> bool;

    fn is_server(&self) -> bool { !self.is_client() } 

    fn get_side_name(&self) -> &str
    {
        if self.is_client() { CLIENT_NAME } else { SERVER_NAME }
    }

    fn get_opposite_side_name(&self) -> &str
    {
        if self.is_server() { CLIENT_NAME } else { SERVER_NAME }
    }

}

pub trait Config: Sided
{
    fn get_ip(&self) -> &String;

    fn get_port(&self) -> &String;

    fn get_logs_path(&self) -> &String;

    fn get_files_path(&self) -> &String;

    fn create_logger(&self) -> Logger;

}

