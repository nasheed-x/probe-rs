# Diode WebUSB branch

The `diode/webusb` branch carries Diode's browser-probe fixes on top of
probe-rs/probe-rs#2958 at commit
`484a7ecd980cf16870f513d01a8210e0272df8b4`.

It contains narrow fixes for CMSIS-DAP discovery and response alignment, RTT
reads at memory-region boundaries, and WebUSB transfer ownership. The `nusb`
directory is based on Yatekii/nusb's `task/webusb` branch at commit
`c9b2c934c86b250b60ca2c18fe130454595f502a`; its upstream license files are
kept alongside the source.

Consumers should depend on this branch by exact commit rather than by branch
name.
