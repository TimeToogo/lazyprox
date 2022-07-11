# lazyprox
Minimal TCP proxy with an exit-on-idle timeout configuration option. Useful for sparing resources on idling devcontainers.

### Usage

```
./target/release/lazyprox --help
lazyprox 0.1.0

USAGE:
    lazyprox --listen <LISTEN> --dest <DEST> --idle-timeout-secs <IDLE_TIMEOUT_SECS>

OPTIONS:
    -d, --dest <DEST>
            Socket address of TCP destination, eg localhost:22

    -h, --help
            Print help information

    -i, --idle-timeout-secs <IDLE_TIMEOUT_SECS>
            Number of seconds of idle time before exiting

    -l, --listen <LISTEN>
            Bind socket address of TCP listener, eg 0.0.0.0:2222

    -V, --version
            Print version information
➜  lazyprox git:(main) 
```