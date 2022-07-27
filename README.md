# byte_slurper

This is the packet capture code for GReX that ingests packetized spectra data
and sends it to a PSRDada buffer.

## Heimdall

Like the rest of the astronomy software we've dealt with, the interface of DADA
buffers to heimdall is undocumented.

Specifically, the header needs the keys:

- FREQ - Center freq (MHz)
- NCHAN - Number of channels
- BW - Bandwidth of observation (MHz)
- NPOL - Number of polarizations
- NBIT - Bits per sample
- TSAMP - Sampling interval (us)
- UTC_START - yyy-mm-dd-hh:mm:ss
