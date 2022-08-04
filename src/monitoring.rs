//! In this module, we implement all the monitoring logic for the packet capture.
//! This includes getting drop data from libpcap as well as various runtime stats.
//! Additionally, we'll hold on to a chunk of average spectra so it can be queried
//! from some TCP listener.
