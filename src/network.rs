use std::{io::{Error, ErrorKind, Read, Write}, net::{TcpListener, TcpStream}};
use crate::{tools::{Logger, Sided}};

const EXIT_CODE_SUCCESS: i32 = 0;
const EXIT_CODE_EMPTY_PACKET: i32 = 1;
const EXIT_CODE_PACKET_NOT_SENT: i32 = 2;
const EXIT_CODE_FILE_NOT_FOUND: i32 = 3;
const EXIT_CODE_IO_ERROR: i32 = 4;
const EXIT_CODE_INVALID_SIDE: i32 = 5;
const EXIT_CODE_INVALID_PACKET_ID: i32 = -1;

pub struct Buffer(Vec<u8>);

impl Buffer
{
    pub fn new() -> Buffer { Buffer(Vec::new()) }

    pub fn read_byte(&mut self) -> u8 { self.0.remove(0) }

    pub fn read_bool(&mut self) -> bool { self.read_byte() != 0 }

    pub fn read_unsigned(&mut self) -> u32
    {
        let bytes: [u8; 4] = self.0.drain(0..4).collect::<Vec<u8>>().try_into().unwrap();
        u32::from_be_bytes(bytes)
    }

    pub fn read_signed(&mut self) -> i32
    {
        let bytes: [u8; 4] = self.0.drain(0..4).collect::<Vec<u8>>().try_into().unwrap();
        i32::from_be_bytes(bytes)
    }

    pub fn read_float(&mut self) -> f32
    {
        let bytes: [u8; 4] = self.0.drain(0..4).collect::<Vec<u8>>().try_into().unwrap();
        f32::from_be_bytes(bytes)
    }

    pub fn read_string(&mut self) -> String
    {
        let length: usize = self.read_unsigned() as usize;
        let bytes: Vec<u8> = self.0.drain(0..length).collect::<Vec<u8>>();
        String::from_utf8(bytes).unwrap()
    }

    pub fn read_bytes(&mut self) -> Vec<u8>
    {
        let length: usize = self.read_unsigned() as usize;
        self.0.drain(0..length).collect::<Vec<u8>>()
    }


    pub fn write_byte(&mut self, byte: u8) { self.0.push(byte); }

    pub fn write_bool(&mut self, b: bool)
    {
        let byte: u8 = if b { 1 } else { 0 };
        self.write_byte(byte);
    }

    pub fn write_unsigned(&mut self, unsigned: u32)
    {
        let bytes: [u8; 4] = unsigned.to_be_bytes();
        self.0.extend(bytes);
    }

    pub fn write_signed(&mut self, signed: i32)
    {
        let bytes: [u8; 4] = signed.to_be_bytes();
        self.0.extend(bytes);
    }

    pub fn write_float(&mut self, float: f32)
    {
        let bytes: [u8; 4] = float.to_be_bytes();
        self.0.extend(bytes);
    }

    pub fn write_string(&mut self, string: String)
    {
        let length: u32 = string.len() as u32;
        let bytes: &[u8] = string.as_bytes();

        self.write_unsigned(length);
        self.0.extend(bytes);
    }
    
    pub fn write_bytes(&mut self, bytes: Vec<u8>)
    {
        let length: u32 = bytes.len() as u32;
        
        self.write_unsigned(length);
        self.0.extend(bytes);
    }


    fn wrap(&mut self)
    {
        let length: u32 = self.0.len() as u32;
        let length_bytes: [u8; 4] = length.to_be_bytes();
        self.0.splice(0..0, length_bytes);
    } 

}

pub trait Packet
{
    fn get_name(&self) -> String;

    fn decode(buffer: &mut Buffer) -> Self where Self: Sized;

    fn encode(&self, buffer: &mut Buffer);

    fn consume(&self, sender: &PacketSender) -> i32; // Return the exit code
}

struct PacketType
{
    name: String, 
    id: u8, 
    decoder: fn(&mut Buffer) -> Box<dyn Packet>
}

impl Clone for PacketType
{
    fn clone(&self) -> Self 
    {
        PacketType 
        {
            name: self.name.clone(), 
            id: self.id, 
            decoder: self.decoder.clone()
        }    
    }
}

pub struct PacketRegistry(Vec<PacketType>);

impl PacketRegistry
{
    pub fn new() -> PacketRegistry { PacketRegistry(Vec::new()) }

    pub fn register<P: Packet + 'static>(&mut self, name: &str)
    {
        let packet_type: PacketType = PacketType
        {
            name: String::from(name),
            id: self.0.len() as u8,
            decoder: |mut buffer| Box::new(P::decode(&mut buffer))
        };

        self.0.push(packet_type);
    }

    fn get_packet_type(&self, id: &u8) -> Option<&PacketType> { self.0.get(*id as usize) }

    fn get_packet_type_for(&self, packet: &dyn Packet) -> Option<&PacketType>
    {
        let name = packet.get_name();

        for packet_type in &self.0
        {
            if packet_type.name == name { return Some(packet_type) }
        }
        None
    }

    fn get_packet_id_for(&self, packet: &dyn Packet) -> Option<u8> 
    {
        match self.get_packet_type_for(packet)
        {
            Some(packet_type) => { Some(packet_type.id) }
            None => None
        }
    }

    fn is_valid_packet(&self, packet: &dyn Packet) -> bool { self.get_packet_type_for(packet).is_some() }
    
    fn is_valid_id(&self, id: &u8) -> bool { (*id as usize) < self.0.len() }


}

impl Clone for PacketRegistry
{
    fn clone(&self) -> Self 
    {
        let mut types: Vec<PacketType> = Vec::new();
        for packet_type in &self.0 { types.push(packet_type.clone()); }

        PacketRegistry(types)
    }

}

pub struct PacketHandler
{
    socket: String,
    client: bool,
    packet_registry: PacketRegistry,
    logger: Logger,
    client_stream: Option<TcpStream>,
    server_listener: Option<TcpListener>
}

impl PacketHandler
{
    pub fn client(ip_v4: &str, port: &str, packet_registry: &PacketRegistry, logger: &Logger) -> PacketHandler
    {
        let socket = format!("{}:{}", &ip_v4, &port);
        PacketHandler
        {
            socket: socket.clone(),
            client: true,
            packet_registry: packet_registry.clone(),
            logger: logger.clone(),
            client_stream: Some(TcpStream::connect(socket.clone()).expect("Failed to connect to the server")),
            server_listener: None
        }
    }

    pub fn server(ip_v4: &str, port: &str, packet_registry: &PacketRegistry, logger: &Logger) -> PacketHandler
    {
        let socket = format!("{}:{}", &ip_v4, &port);
        PacketHandler
        {
            socket: socket.clone(),
            client: false,
            packet_registry: packet_registry.clone(),
            logger: logger.clone(),
            client_stream: None,
            server_listener: Some(TcpListener::bind(socket.clone()).expect("Failed to setup the server")),
        }
    }

    fn receive(&self, stream: &mut TcpStream) -> std::io::Result<i32>
    {
        let exit_code = self.receive_internal(stream)?;

        match exit_code
        {
            EXIT_CODE_SUCCESS =>           { return Ok(exit_code); }
            EXIT_CODE_EMPTY_PACKET =>      { self.logger.error_literal(format!("Received an empty packet from the {}", self.get_opposite_side_name())); }
            EXIT_CODE_FILE_NOT_FOUND =>    { self.logger.error("File not found"); }
            EXIT_CODE_IO_ERROR =>          { self.logger.error("Error reading or writing files"); }
            EXIT_CODE_INVALID_SIDE =>      { self.logger.error_literal(format!("Received a {}-to-{} packet from the {}", self.get_side_name(), self.get_opposite_side_name(), self.get_opposite_side_name())); }
            EXIT_CODE_INVALID_PACKET_ID => { self.logger.error_literal(format!("Received an invalid packet from the {}", self.get_opposite_side_name())); }
            other =>                  { self.logger.error_literal(format!("A packet returned an unknown error code {}", other)); }
        }
        Err(Error::new(ErrorKind::InvalidData, "Cannot read the packet"))
    }

    fn receive_internal(&self, stream: &mut TcpStream) -> std::io::Result<i32> // Return the exit code
    {
        let mut id_bytes = [0u8; 1];
        let mut len_bytes = [0u8; 4];

        match stream.read_exact(&mut id_bytes)
        {
            Ok(_) => {}
            Err(_) => { return Ok(EXIT_CODE_SUCCESS); } // If no byte is present, assume no error
        }

        stream.read_exact(&mut len_bytes)?;

        let packet_id: u8 = id_bytes[0];
        let length = u32::from_be_bytes(len_bytes) as usize;

        if length == 0 { return Ok(EXIT_CODE_EMPTY_PACKET); }

        let mut body: Vec<u8> = vec![0u8; length];
        stream.read_exact(&mut body)?;


        if !self.packet_registry.is_valid_id(&packet_id) { return Ok(EXIT_CODE_INVALID_PACKET_ID); }

        let mut buffer = Buffer(body);
        let packet_box: Box<dyn Packet> =     (self.packet_registry.get_packet_type(&packet_id).unwrap().decoder)(&mut buffer);
        
        let packet_sender = self.create_sender_from_stream(stream)?;

        Ok(packet_box.consume(&packet_sender))
    }

    pub fn create_sender(&self) -> std::io::Result<PacketSender>
    {
        if self.is_server() { Err(Error::new(ErrorKind::NetworkUnreachable, "Cannot create a sender without a stream")) }
        else { self.create_sender_from_stream(self.client_stream.as_ref().unwrap()) }
    }

    fn create_sender_from_stream(&self, stream: &TcpStream) -> std::io::Result<PacketSender> 
    {
        Ok(PacketSender 
            {
                logger: self.logger.clone(), 
                packet_registry: self.packet_registry.clone(), 
                stream: stream.try_clone()?, 
                client: self.client
            })
    }
    
    pub fn run(&mut self)
    {
        if self.is_client() { self.run_client(); }
        else                { self.run_server(); }
    }

    fn run_client(&mut self)
    {
        self.logger.info("Client packet handler is listening");

        if let Some(mut stream) = self.client_stream.take() 
        {
            loop 
            {
                match self.receive(&mut stream) 
                {
                    Ok(_) => {} // Move on
                    Err(_) => { break; } // Break the loop
                }
            }
        
            self.client_stream = Some(stream);
        }
    }

    fn run_server(&mut self)
    {
        self.logger.info("Server packet handler is listening");
        let listener = self.server_listener.as_ref().unwrap();

        loop 
        {
            let mut stream = match listener.accept() 
            {
                Ok((stream, _)) => stream,
                Err(e) => {
                    self.logger.error_literal(format!("Connection failed: {}", e));
                    continue; // Try accepting again.
                }
            };

            self.handle_client(&mut stream)
        }
    }

    fn handle_client(&self, stream: &mut TcpStream)
    {
        loop 
        {
            match self.receive(stream) 
            {
                Ok(_) => {}
                Err(error) => { self.logger.error_literal(format!("Client error: {}", error)); }
            }
        }

    }
}




impl Sided for PacketHandler
{
    fn is_client(&self) -> bool { self.client }
}


pub struct PacketSender
{
    pub logger: Logger,
    packet_registry: PacketRegistry,
    stream: TcpStream,
    client: bool,
}

impl PacketSender
{
    pub fn send(&mut self, packet: &dyn Packet) -> std::io::Result<i32>
    {
        let exit_code = self.send_internal(packet)?;

        match exit_code
        {
            EXIT_CODE_SUCCESS => { return Ok(exit_code); }
            EXIT_CODE_INVALID_PACKET_ID => 
            {   
                self.logger.error_literal(format!("Attempted to send an invalid packet to the {}", self.get_opposite_side_name()));
            }
            other =>                  
            {                
                self.logger.error_literal(format!("A packet returned an unknown error code {}", other));
            }
        }

        Err(Error::new(ErrorKind::Other, "Cannot write the packet"))
    }

    fn send_internal(&mut self, packet: &dyn Packet) -> std::io::Result<i32>
    {
        let mut stream = &self.stream;
        let mut buffer = Buffer::new();
        let full_length;
        let packet_id_option: Option<u8> = self.packet_registry.get_packet_id_for(packet);

        match packet_id_option
        {
            None => { return Ok(EXIT_CODE_INVALID_PACKET_ID); }
            Some(packet_id) =>
            {
                packet.encode(&mut buffer);
                buffer.wrap();
                buffer.0.insert(0, packet_id);

                full_length = buffer.0.len();
            }
        }

        stream.write_all(&buffer.0[0..full_length])?;
        stream.flush()?;
                
        Ok(EXIT_CODE_SUCCESS)
    }

}

impl Sided for PacketSender
{
    fn is_client(&self) -> bool { self.client }        
}



