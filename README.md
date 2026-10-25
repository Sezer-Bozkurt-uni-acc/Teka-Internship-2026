=== COPIED FROM MY INTERNSHIP REPORT ===

My project is a server infrastructure coded entirely in Rust. 
The .exe file (application) contains the code for both the server and the client. 
A configuration file (config.toml) has to be placed on the same folder with the .exe file for the system to detect the server/client. 
The server and the client can run on the same device. 
In fact, even though we tested this project on a multi-device setup, I did most of my debugging in my computer only. 
The configuration file must contain the IP address, the port, the folder paths for the logs/.hex files.

The system, like any other, sends information in packets. 
Each packet is registered by strings and has their own encode/decode functions. 
I included a Buffer class to make transferring simple variables like strings, integers, bytes, floats simpler.

The system works with commands, which are also registered by strings.
Commands are always run from the client and they usually send packets to the server.
The server receives the packets and may send response packets when necessary. 
When my coworkers continue this project by coding the UI, they could attach the UI elements to commands, which are just strings.
