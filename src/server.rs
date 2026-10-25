use core::error;
use std::{collections::HashMap, env, fs::{self, OpenOptions}, io::{Error, ErrorKind::NotFound, Write}};
use crate::{client::ClientConfig, tools::{self, Config, Logger, Sided}};

const MAPPING_FILE_NAME: &str = "mappings.txt";

const EXIT_CODE_SUCCESS: i32 = 0;
const EXIT_CODE_FILE_NOT_FOUND: i32 = 3;
const EXIT_CODE_IO_ERROR: i32 = 4;

pub struct ServerConfig
{
    ip: String,
    port: String,
    logs: String,
    files: String
}

impl ServerConfig
{
    pub fn get() -> Option<ServerConfig> 
    {
        match tools::create_string_map("config.toml", "server")
        {
            Ok(map) => { Some(ServerConfig::from_map(map)) }
            Err(_) => { None }
        }
    }

    pub fn get_or_throw() -> ServerConfig { ServerConfig::get().unwrap() }

    fn from_map(map: HashMap<String, String>) -> ServerConfig
    {
        ServerConfig 
        {
            ip:   map.get("ip").expect("Server IP address not found").clone(), 
            port: map.get("port").unwrap_or(&"8080".to_string()).clone(), 
            logs: map.get("logs").expect("Server log folder not found").clone(),
            files: map.get("files").expect("Server folder not found").clone()
        }
    }

}

impl Config for ServerConfig
{
    fn get_ip(&self) -> &String { &self.ip }

    fn get_port(&self) -> &String { &self.port }

    fn get_logs_path(&self) -> &String { &self.logs }

    fn get_files_path(&self) -> &String { &self.files }

    fn create_logger(&self) -> Logger { Logger::create("SERVER", &self.logs) }
}

impl Sided for ServerConfig
{
    fn is_client(&self) -> bool { false }
}


pub struct FileMapping(ServerConfig, HashMap<String, String>);

impl FileMapping
{
    pub fn get() -> std::io::Result<FileMapping>
    {
        let server_config_option = ServerConfig::get();

        if server_config_option.is_none() { return Err(Error::new(NotFound, "Server not found")); }

        let mut mapping = FileMapping(server_config_option.unwrap(), HashMap::new());

        if let Ok(exe_path) = env::current_exe()
        {
            let directory = exe_path.parent().expect("Error locating parent directory");
            let path = directory.join(MAPPING_FILE_NAME);

            if let Ok(contents) = fs::read_to_string(path)
            {
                for line in contents.lines()
                { 
                    let mut parts = line.splitn(2, ":");
                    let processor_name = parts.next().map(|slice| String::from(slice)).unwrap_or(String::new());
                    let file_name      = parts.next().map(|slice| String::from(slice)).unwrap_or(String::new());
                    
                    if !processor_name.is_empty() && !file_name.is_empty()
                    {
                        mapping.insert(processor_name, file_name);
                    }
                }
            } 
        }

        Ok(mapping)

    }

    pub fn map(processor_name: String, file_name: String) -> std::io::Result<i32>
    {
        let result = FileMapping::get();
        if result.is_err() { return Ok(EXIT_CODE_FILE_NOT_FOUND); }
    
        let mut mapping = result.unwrap();
        
        if mapping.insert(processor_name, file_name) { mapping.save_to_file() }
        else                                         { return Ok(EXIT_CODE_FILE_NOT_FOUND); }
    }

    pub fn clear()
    {
        if ServerConfig::get().is_none() { return; }

        match env::current_exe()
        {
            Ok(exe_path) =>
            {
                let directory = exe_path.parent().expect("Error locating parent directory");
                let path = directory.join(MAPPING_FILE_NAME);

                if path.exists()
                {
                    let mut file = OpenOptions::new().create(true).write(true).truncate(true).open(path).expect("Error opening mapping file");
                    let _ = file.write_all("".as_bytes());
                }
            }
            Err(_) => {}
        }
    }

    pub fn get_file_name_for(&self, processor_name: &str) -> Option<String> { self.1.get(processor_name).cloned() }

    pub fn get_file_data_for(&self, processor_name: &str) -> Option<(String, Vec<u8>)>
    {
        match self.get_file_name_for(processor_name)
        {
            Some(file_name) =>
            {
                let path = format!("{}/{}", self.0.get_files_path(), &file_name);

                match fs::read(path)
                {
                    Ok(bytes) => { Some((file_name, bytes)) }
                    Err(_) => { None }
                }

            }
            None => { None }
        }

    }

    // Returns whether the file exists or not
    fn insert(&mut self, processor_name: String, file_name: String) -> bool
    {
        let file_path = format!("{}/{}", self.0.get_files_path(), &file_name);

        match fs::exists(file_path)
        {
            Ok(exists) =>
            {
                if exists
                {
                    self.1.insert(processor_name, file_name);
                    true
                }
                else { false }
            }
            Err(_) => { false }
        }
    }

    fn save_to_file(&self) -> std::io::Result<i32>
    {
        match env::current_exe()
        {
            Ok(exe_path) =>
            {
                let directory = exe_path.parent().expect("Error locating parent directory");
                let path = directory.join(MAPPING_FILE_NAME);
                let mut file = OpenOptions::new().create(true).write(true).open(path)?;
                let mut contents = String::new();

                for (processor_name, file_name) in self.1.iter()
                {
                    contents.push_str(processor_name);
                    contents.push_str(":");
                    contents.push_str(file_name);
                    contents.push_str("\n");
                }

                file.write_all(contents.as_bytes())?;
                Ok(EXIT_CODE_SUCCESS)
            }
            Err(_) => { Ok(EXIT_CODE_IO_ERROR) }
        }
    }


}

