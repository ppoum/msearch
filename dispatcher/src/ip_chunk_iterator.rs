use std::net::Ipv4Addr;
use std::num::Wrapping;
use rand::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct IpChunkIterator {
    offset: u32,
    x: u32,
}

impl IpChunkIterator {
    pub fn new() -> Self {
        IpChunkIterator {
            offset: thread_rng().gen_range(2..1000000),
            x: 0
        }
    }

    /// Regenerates the *IpChunkIterator* with a new random seed, such that the next
    /// sequences of IPs are in a different order
    pub fn regenerate(&mut self) {
        self.x = 0;
        self.offset = thread_rng().gen_range(2..1000000);
    }

    fn mapped_x(&self) -> u32 {
        // 1:1 unique stable map
        let mut value = Wrapping(self.x);
        value *= 1664525;
        value += 1013904223;
        value *= self.offset;
        value ^= value >> 12;
        value ^= value << 25;
        value ^= value >> 27;
        value *= 1103515245;
        value += 12345;

        value.0
    }
}

impl Iterator for IpChunkIterator {
    type Item = Ipv4Addr;

    fn next(&mut self) -> Option<Self::Item> {
        if self.x == u32::MAX {
            return None
        }

        let mut result = None;
        while self.x != u32::MAX {
            // Use the value of mapped x, which is a 1:1 stable map on 32 bits
            // This allows us to traverse IPs in a pseudo random order
            let ip = Ipv4Addr::from(self.mapped_x());
            self.x += 1;

            // Check that IP is accessible publicly. Switch to .is_global when stable
            if !ip.is_loopback() && !ip.is_private() && !ip.is_link_local() && !ip.is_multicast()
                && !ip.is_broadcast() && !ip.is_documentation() {
                result = Some(ip);
                break;
            }
        }

        result
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (u32::MAX - self.x) as usize;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for IpChunkIterator {

}
