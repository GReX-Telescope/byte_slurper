# byte_slurper

This is the packet capture code for GReX that ingests packetized spectra data
and sends it to a PSRDada buffer.

Documentation to come!

```sh
byte_slurper 0.1.0

USAGE:
    byte_slurper [OPTIONS] --key <KEY> --device-name <DEVICE_NAME>

OPTIONS:
    -c, --capacity <CAPACITY>          Ring buffer capacity [default: 256]
    -d, --device-name <DEVICE_NAME>    Network device to capture packets from (MTU must be set to 9000)
    -h, --help                         Print help information
    -k, --key <KEY>                    Hexadecimal (sans leading 0x) PSRDADA key to create as source for heimdall
    -l, --listen-port <LISTEN_PORT>    Port to send TCP average spectra to [default: 4242]
    -p, --port <PORT>                  Port to capture UDP data from [default: 60000]
    -q, --quiet                        Less output per occurrence
    -v, --verbose                      More output per occurrence
    -V, --version                      Print version information
```
