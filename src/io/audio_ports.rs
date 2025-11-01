use anyhow::{Context, Result};
use jack::{AudioIn, AudioOut, Client, Port, ProcessScope};

pub struct AudioPorts {
    input: Port<AudioIn>,
    output_left: Port<AudioOut>,
    output_right: Port<AudioOut>,
}

impl AudioPorts {
    pub fn new(client: &Client) -> Result<Self> {
        Ok(Self {
            input: client
                .register_port("in_port", AudioIn::default())
                .context("failed to register in port")?,
            output_left: client
                .register_port("out_port_left", AudioOut::default())
                .context("failed to register out port left")?,
            output_right: client
                .register_port("out_port_right", AudioOut::default())
                .context("failed to register out port right")?,
        })
    }

    pub fn read_input<'a>(&'a self, ps: &'a ProcessScope) -> &'a [f32] {
        self.input.as_slice(ps)
    }

    pub fn write_output(&mut self, ps: &ProcessScope, samples: &[f32]) {
        let output_size = ps.n_frames() as usize;
        let frame_count = samples.len().min(output_size);
        let out_left = self.output_left.as_mut_slice(ps);
        let out_right = self.output_right.as_mut_slice(ps);

        out_left[..frame_count].copy_from_slice(&samples[..frame_count]);
        out_right[..frame_count].copy_from_slice(&samples[..frame_count]);

        for i in frame_count..output_size {
            out_left[i] = 0.0;
            out_right[i] = 0.0;
        }
    }

    pub fn silence_output(&mut self, ps: &ProcessScope) {
        let output_size = ps.n_frames() as usize;
        let out_left = self.output_left.as_mut_slice(ps);
        let out_right = self.output_right.as_mut_slice(ps);
        out_left[..output_size].fill(0.0);
        out_right[..output_size].fill(0.0);
    }
}
