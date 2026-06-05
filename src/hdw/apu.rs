/*
    Game Boy Audio Processing Unit (APU)

    Based on Pan Docs: https://gbdev.io/pandocs/Audio.html

    The Game Boy APU has 4 sound channels:
    - Channel 1: Square wave with sweep
    - Channel 2: Square wave
    - Channel 3: Arbitrary wave
    - Channel 4: Noise
*/

// Audio channel envelope for volume control
#[derive(Debug, Clone)]
pub struct Envelope {
    pub initial_volume: u8,
    pub direction: bool, // true = increase, false = decrease
    pub step_length: u8,
    pub volume: u8,
    pub timer: u8,
}

impl Envelope {
    pub fn new() -> Self {
        Envelope {
            initial_volume: 0,
            direction: false,
            step_length: 0,
            volume: 0,
            timer: 0,
        }
    }

    pub fn trigger(&mut self) {
        self.volume = self.initial_volume;
        self.timer = self.step_length;
    }

    pub fn tick(&mut self) {
        if self.step_length == 0 {
            return;
        }

        if self.timer > 0 {
            self.timer -= 1;
        }

        if self.timer == 0 {
            self.timer = self.step_length;

            if self.direction && self.volume < 15 {
                self.volume += 1;
            } else if !self.direction && self.volume > 0 {
                self.volume -= 1;
            }
        }
    }
}

// Length timer for automatic channel shutdown
#[derive(Debug, Clone)]
pub struct LengthTimer {
    pub length: u16, // Changed to u16 to accommodate 256 - value
    pub enabled: bool,
    pub max_length: u16, // Changed to u16 to accommodate 256
}

impl LengthTimer {
    pub fn new(max_length: u16) -> Self {
        // Changed parameter to u16
        LengthTimer {
            length: 0,
            enabled: false,
            max_length,
        }
    }

    pub fn trigger(&mut self) {
        if self.length == 0 {
            self.length = self.max_length; // Now both are u16
        }
    }

    pub fn tick(&mut self) -> bool {
        if self.enabled && self.length > 0 {
            self.length -= 1;
            self.length == 0
        } else {
            false
        }
    }
}

// Frequency sweep for Channel 1
#[derive(Debug, Clone)]
pub struct FrequencySweep {
    pub shift: u8,
    pub direction: bool, // true = increase, false = decrease
    pub time: u8,
    pub timer: u8,
    pub enabled: bool,
    pub shadow_frequency: u16,
}

impl FrequencySweep {
    pub fn new() -> Self {
        FrequencySweep {
            shift: 0,
            direction: false,
            time: 0,
            timer: 0,
            enabled: false,
            shadow_frequency: 0,
        }
    }

    pub fn trigger(&mut self, frequency: u16) {
        self.shadow_frequency = frequency;
        self.timer = if self.time > 0 { self.time } else { 8 };
        self.enabled = self.time > 0 || self.shift > 0;
    }

    pub fn tick(&mut self) -> Option<u16> {
        if self.timer > 0 {
            self.timer -= 1;
        }

        if self.timer == 0 {
            self.timer = if self.time > 0 { self.time } else { 8 };

            if self.enabled && self.time > 0 {
                let new_frequency = self.calculate_frequency();
                if new_frequency <= 2047 && self.shift > 0 {
                    self.shadow_frequency = new_frequency;
                    return Some(new_frequency);
                }
            }
        }
        None
    }

    fn calculate_frequency(&self) -> u16 {
        let offset = self.shadow_frequency >> self.shift;
        if self.direction {
            self.shadow_frequency.saturating_sub(offset)
        } else {
            self.shadow_frequency.saturating_add(offset)
        }
    }
}

// Square/Pulse wave channel (CH1 and CH2)
#[derive(Debug, Clone)]
pub struct SquareChannel {
    pub enabled: bool,
    pub dac_enabled: bool,
    pub frequency: u16,
    pub duty_cycle: u8,
    pub envelope: Envelope,
    pub length_timer: LengthTimer,
    pub sweep: Option<FrequencySweep>, // Only CH1 has sweep

    // Internal state
    pub frequency_timer: u16,
    pub duty_position: u8,
}

impl SquareChannel {
    pub fn new(has_sweep: bool) -> Self {
        SquareChannel {
            enabled: false,
            dac_enabled: false,
            frequency: 0,
            duty_cycle: 0,
            envelope: Envelope::new(),
            length_timer: LengthTimer::new(64),
            sweep: if has_sweep {
                Some(FrequencySweep::new())
            } else {
                None
            },
            frequency_timer: 0,
            duty_position: 0,
        }
    }

    pub fn trigger(&mut self) {
        self.enabled = true;
        self.length_timer.trigger();
        self.envelope.trigger();

        if let Some(sweep) = &mut self.sweep {
            sweep.trigger(self.frequency);
        }

        self.frequency_timer = (2048 - self.frequency) * 4;
    }

    pub fn step(&mut self) {
        if self.frequency_timer > 0 {
            self.frequency_timer -= 1;
        } else {
            self.frequency_timer = (2048 - self.frequency) * 4;
            self.duty_position = (self.duty_position + 1) % 8;
        }
    }

    pub fn get_output(&self) -> u8 {
        if !self.enabled || !self.dac_enabled {
            return 0;
        }

        let duty_patterns = [
            0b00000001, // 12.5%
            0b10000001, // 25%
            0b10000111, // 50%
            0b01111110, // 75%
        ];

        let pattern = duty_patterns[self.duty_cycle as usize];
        let bit = (pattern >> self.duty_position) & 1;

        if bit != 0 {
            self.envelope.volume
        } else {
            0
        }
    }

    pub fn length_tick(&mut self) {
        if self.length_timer.tick() {
            self.enabled = false;
        }
    }

    pub fn envelope_tick(&mut self) {
        self.envelope.tick();
    }

    pub fn sweep_tick(&mut self) {
        if let Some(sweep) = &mut self.sweep {
            if let Some(new_freq) = sweep.tick() {
                self.frequency = new_freq;
            }
        }
    }
}

// Wave channel (CH3)
#[derive(Debug, Clone)]
pub struct WaveChannel {
    pub enabled: bool,
    pub dac_enabled: bool,
    pub frequency: u16,
    pub volume: u8,
    pub length_timer: LengthTimer,
    pub wave_ram: [u8; 16],

    // Internal state
    pub frequency_timer: u16,
    pub wave_position: u8,
}

impl WaveChannel {
    pub fn new() -> Self {
        WaveChannel {
            enabled: false,
            dac_enabled: false,
            frequency: 0,
            volume: 0,
            length_timer: LengthTimer::new(256),
            wave_ram: [0; 16],
            frequency_timer: 0,
            wave_position: 0,
        }
    }

    pub fn trigger(&mut self) {
        self.enabled = true;
        self.length_timer.trigger();
        self.frequency_timer = (2048 - self.frequency) * 2;
        self.wave_position = 0;
    }

    pub fn step(&mut self) {
        if self.frequency_timer > 0 {
            self.frequency_timer -= 1;
        } else {
            self.frequency_timer = (2048 - self.frequency) * 2;
            self.wave_position = (self.wave_position + 1) % 32;
        }
    }

    pub fn get_output(&self) -> u8 {
        if !self.enabled || !self.dac_enabled {
            return 0;
        }

        let byte_index = (self.wave_position / 2) as usize;
        let nibble = if self.wave_position % 2 == 0 {
            (self.wave_ram[byte_index] >> 4) & 0xF
        } else {
            self.wave_ram[byte_index] & 0xF
        };

        match self.volume {
            0 => 0,           // Mute
            1 => nibble,      // 100%
            2 => nibble >> 1, // 50%
            3 => nibble >> 2, // 25%
            _ => 0,
        }
    }

    pub fn length_tick(&mut self) {
        if self.length_timer.tick() {
            self.enabled = false;
        }
    }
}

// Noise channel (CH4)
#[derive(Debug, Clone)]
pub struct NoiseChannel {
    pub enabled: bool,
    pub dac_enabled: bool,
    pub clock_shift: u8,
    pub width_mode: bool, // false = 15-bit, true = 7-bit
    pub divisor_code: u8,
    pub envelope: Envelope,
    pub length_timer: LengthTimer,

    // Internal state
    pub frequency_timer: u16,
    pub lfsr: u16,
}

impl NoiseChannel {
    pub fn new() -> Self {
        NoiseChannel {
            enabled: false,
            dac_enabled: false,
            clock_shift: 0,
            width_mode: false,
            divisor_code: 0,
            envelope: Envelope::new(),
            length_timer: LengthTimer::new(64),
            frequency_timer: 0,
            lfsr: 0x7FFF,
        }
    }

    pub fn trigger(&mut self) {
        self.enabled = true;
        self.length_timer.trigger();
        self.envelope.trigger();
        self.lfsr = 0x7FFF;

        let divisor = if self.divisor_code == 0 {
            8
        } else {
            (self.divisor_code as u16) << 4
        };
        self.frequency_timer = divisor << self.clock_shift;
    }

    pub fn step(&mut self) {
        if self.frequency_timer > 0 {
            self.frequency_timer -= 1;
        } else {
            let divisor = if self.divisor_code == 0 {
                8
            } else {
                (self.divisor_code as u16) << 4
            };
            self.frequency_timer = divisor << self.clock_shift;

            let bit = (self.lfsr & 1) ^ ((self.lfsr >> 1) & 1);
            self.lfsr = (self.lfsr >> 1) | (bit << 14);

            if self.width_mode {
                self.lfsr = (self.lfsr & !0x40) | (bit << 6);
            }
        }
    }

    pub fn get_output(&self) -> u8 {
        if !self.enabled || !self.dac_enabled {
            return 0;
        }

        if (self.lfsr & 1) == 0 {
            self.envelope.volume
        } else {
            0
        }
    }

    pub fn length_tick(&mut self) {
        if self.length_timer.tick() {
            self.enabled = false;
        }
    }

    pub fn envelope_tick(&mut self) {
        self.envelope.tick();
    }
}

// Main APU struct
pub struct AudioSystem {
    pub channel1: SquareChannel,
    pub channel2: SquareChannel,
    pub channel3: WaveChannel,
    pub channel4: NoiseChannel,

    // Master controls
    pub master_enable: bool,
    pub left_volume: u8,
    pub right_volume: u8,
    pub left_enables: u8,  // Which channels are enabled for left output
    pub right_enables: u8, // Which channels are enabled for right output

    // Frame sequencer for timing envelope, length, and sweep
    pub frame_sequencer: u8,
    pub frame_sequencer_timer: u16,

    // Sample buffer for audio output
    pub sample_buffer: Vec<f32>,
    sample_rate_counter: u16,
}

impl AudioSystem {
    pub fn new() -> Self {
        AudioSystem {
            channel1: SquareChannel::new(true),  // CH1 has sweep
            channel2: SquareChannel::new(false), // CH2 no sweep
            channel3: WaveChannel::new(),
            channel4: NoiseChannel::new(),
            master_enable: false,
            left_volume: 0,
            right_volume: 0,
            left_enables: 0,
            right_enables: 0,
            frame_sequencer: 0,
            frame_sequencer_timer: 8192, // 512 Hz timer
            sample_buffer: Vec::new(),
            sample_rate_counter: 0,
        }
    }

    pub fn tick(&mut self) {
        // Step frame sequencer (controls envelope, length, and sweep timing)
        if self.frame_sequencer_timer > 0 {
            self.frame_sequencer_timer -= 1;
        } else {
            self.frame_sequencer_timer = 8192;
            self.tick_frame_sequencer();
        }

        // Step all channels
        self.channel1.step();
        self.channel2.step();
        self.channel3.step();
        self.channel4.step();

        // Generate audio samples at ~44.1kHz
        // Game Boy CPU runs at ~4.19MHz, so we sample every ~95 cycles
        self.sample_rate_counter += 1;
        if self.sample_rate_counter >= 95 {
            self.sample_rate_counter = 0;
            self.generate_sample();
        }
    }

    fn generate_sample(&mut self) {
        let (left_sample, right_sample) = self.get_sample_values();

        // Convert to f32 and normalize to -1.0 to 1.0 range
        let left_f32 = (left_sample as f32) / 32768.0;
        let right_f32 = (right_sample as f32) / 32768.0;

        // Add stereo samples to buffer (interleaved)
        self.sample_buffer.push(left_f32);
        self.sample_buffer.push(right_f32);

        // Keep buffer size reasonable
        if self.sample_buffer.len() > 8192 {
            self.sample_buffer.drain(0..2048);
        }
    }

    fn get_sample_values(&self) -> (i16, i16) {
        if !self.master_enable {
            return (0, 0);
        }

        let ch1_out = self.channel1.get_output() as i16;
        let ch2_out = self.channel2.get_output() as i16;
        let ch3_out = self.channel3.get_output() as i16;
        let ch4_out = self.channel4.get_output() as i16;

        let mut left_sample = 0i16;
        let mut right_sample = 0i16;

        if (self.left_enables & 0x01) != 0 {
            left_sample += ch1_out;
        }
        if (self.left_enables & 0x02) != 0 {
            left_sample += ch2_out;
        }
        if (self.left_enables & 0x04) != 0 {
            left_sample += ch3_out;
        }
        if (self.left_enables & 0x08) != 0 {
            left_sample += ch4_out;
        }

        if (self.right_enables & 0x01) != 0 {
            right_sample += ch1_out;
        }
        if (self.right_enables & 0x02) != 0 {
            right_sample += ch2_out;
        }
        if (self.right_enables & 0x04) != 0 {
            right_sample += ch3_out;
        }
        if (self.right_enables & 0x08) != 0 {
            right_sample += ch4_out;
        }

        // Apply master volume (0-7 scale to 0-1 scale)
        left_sample = (left_sample * (self.left_volume as i16 + 1)) / 8;
        right_sample = (right_sample * (self.right_volume as i16 + 1)) / 8;

        // Scale to 16-bit range
        left_sample *= 512;
        right_sample *= 512;

        (left_sample, right_sample)
    }

    pub fn get_samples(&mut self, buffer: &mut [f32]) {
        let available = self.sample_buffer.len().min(buffer.len());

        if available > 0 {
            // Copy samples from our buffer to the provided buffer
            buffer[..available].copy_from_slice(&self.sample_buffer[..available]);
            // Remove the samples we just copied
            self.sample_buffer.drain(..available);
        }

        // Fill remaining with silence if needed
        for i in available..buffer.len() {
            buffer[i] = 0.0;
        }
    }

    fn tick_frame_sequencer(&mut self) {
        // Length counter (ticked at 256 Hz)
        if self.frame_sequencer % 2 == 0 {
            self.channel1.length_tick();
            self.channel2.length_tick();
            self.channel3.length_tick();
            self.channel4.length_tick();
        }

        // Envelope (ticked at 64 Hz)
        if self.frame_sequencer == 7 {
            self.channel1.envelope_tick();
            self.channel2.envelope_tick();
            self.channel4.envelope_tick();
        }

        // Sweep (ticked at 128 Hz)
        if self.frame_sequencer == 2 || self.frame_sequencer == 6 {
            self.channel1.sweep_tick();
        }

        self.frame_sequencer = (self.frame_sequencer + 1) % 8;
    }

    // Register read/write functions
    pub fn read_register(&self, address: u16) -> u8 {
        match address {
            // Channel 1 registers
            0xFF10 => {
                // NR10 - Sweep
                if let Some(sweep) = &self.channel1.sweep {
                    0x80 | (sweep.time << 4)
                        | (if sweep.direction { 0x08 } else { 0 })
                        | sweep.shift
                } else {
                    0xFF
                }
            }
            0xFF11 => 0x3F | (self.channel1.duty_cycle << 6), // NR11 - Duty/Length
            0xFF12 => {
                // NR12 - Envelope
                (self.channel1.envelope.initial_volume << 4)
                    | (if self.channel1.envelope.direction {
                        0x08
                    } else {
                        0
                    })
                    | self.channel1.envelope.step_length
            }
            0xFF13 => 0xFF, // NR13 - Frequency low (write-only)
            0xFF14 => {
                // NR14 - Frequency high/Control
                0xBF | (if self.channel1.length_timer.enabled {
                    0x40
                } else {
                    0
                })
            }

            // Channel 2 registers
            0xFF16 => 0x3F | (self.channel2.duty_cycle << 6), // NR21 - Duty/Length
            0xFF17 => {
                // NR22 - Envelope
                (self.channel2.envelope.initial_volume << 4)
                    | (if self.channel2.envelope.direction {
                        0x08
                    } else {
                        0
                    })
                    | self.channel2.envelope.step_length
            }
            0xFF18 => 0xFF, // NR23 - Frequency low (write-only)
            0xFF19 => {
                // NR24 - Frequency high/Control
                0xBF | (if self.channel2.length_timer.enabled {
                    0x40
                } else {
                    0
                })
            }

            // Channel 3 registers
            0xFF1A => 0x7F | (if self.channel3.dac_enabled { 0x80 } else { 0 }), // NR30 - DAC
            0xFF1B => 0xFF, // NR31 - Length (write-only)
            0xFF1C => 0x9F | (self.channel3.volume << 5), // NR32 - Volume
            0xFF1D => 0xFF, // NR33 - Frequency low (write-only)
            0xFF1E => {
                // NR34 - Frequency high/Control
                0xBF | (if self.channel3.length_timer.enabled {
                    0x40
                } else {
                    0
                })
            }

            // Channel 4 registers
            0xFF20 => 0xFF, // NR41 - Length (write-only)
            0xFF21 => {
                // NR42 - Envelope
                (self.channel4.envelope.initial_volume << 4)
                    | (if self.channel4.envelope.direction {
                        0x08
                    } else {
                        0
                    })
                    | self.channel4.envelope.step_length
            }
            0xFF22 => {
                // NR43 - Frequency/Randomness
                (self.channel4.clock_shift << 4)
                    | (if self.channel4.width_mode { 0x08 } else { 0 })
                    | self.channel4.divisor_code
            }
            0xFF23 => {
                // NR44 - Control
                0xBF | (if self.channel4.length_timer.enabled {
                    0x40
                } else {
                    0
                })
            }

            // Master control registers
            0xFF24 => {
                // NR50 - Master volume
                (self.left_volume << 4) | self.right_volume
            }
            0xFF25 => {
                // NR51 - Sound panning
                (self.left_enables << 4) | self.right_enables
            }
            0xFF26 => {
                // NR52 - Master control/status
                (if self.master_enable { 0x80 } else { 0 })
                    | 0x70
                    | (if self.channel4.enabled { 0x08 } else { 0 })
                    | (if self.channel3.enabled { 0x04 } else { 0 })
                    | (if self.channel2.enabled { 0x02 } else { 0 })
                    | (if self.channel1.enabled { 0x01 } else { 0 })
            }

            // Wave RAM
            0xFF30..=0xFF3F => self.channel3.wave_ram[(address - 0xFF30) as usize],

            _ => 0xFF,
        }
    }

    pub fn write_register(&mut self, address: u16, value: u8) {
        if !self.master_enable && address != 0xFF26 {
            return; // Can only write to NR52 when APU is disabled
        }

        match address {
            // Channel 1 registers
            0xFF10 => {
                // NR10 - Sweep
                if let Some(sweep) = &mut self.channel1.sweep {
                    sweep.time = (value >> 4) & 0x07;
                    sweep.direction = (value & 0x08) != 0;
                    sweep.shift = value & 0x07;
                }
            }
            0xFF11 => {
                // NR11 - Duty/Length
                self.channel1.duty_cycle = (value >> 6) & 0x03;
                self.channel1.length_timer.length = (64 - (value & 0x3F)) as u16;
            }
            0xFF12 => {
                // NR12 - Envelope
                self.channel1.envelope.initial_volume = (value >> 4) & 0x0F;
                self.channel1.envelope.direction = (value & 0x08) != 0;
                self.channel1.envelope.step_length = value & 0x07;
                self.channel1.dac_enabled = (value & 0xF8) != 0;
                if !self.channel1.dac_enabled {
                    self.channel1.enabled = false;
                }
            }
            0xFF13 => {
                // NR13 - Frequency low
                self.channel1.frequency = (self.channel1.frequency & 0x0700) | (value as u16);
            }
            0xFF14 => {
                // NR14 - Frequency high/Control
                self.channel1.frequency =
                    (self.channel1.frequency & 0x00FF) | (((value & 0x07) as u16) << 8);
                self.channel1.length_timer.enabled = (value & 0x40) != 0;
                if (value & 0x80) != 0 {
                    self.channel1.trigger();
                }
            }

            // Channel 2 registers
            0xFF16 => {
                // NR21 - Duty/Length
                self.channel2.duty_cycle = (value >> 6) & 0x03;
                self.channel2.length_timer.length = (64 - (value & 0x3F)) as u16;
            }
            0xFF17 => {
                // NR22 - Envelope
                self.channel2.envelope.initial_volume = (value >> 4) & 0x0F;
                self.channel2.envelope.direction = (value & 0x08) != 0;
                self.channel2.envelope.step_length = value & 0x07;
                self.channel2.dac_enabled = (value & 0xF8) != 0;
                if !self.channel2.dac_enabled {
                    self.channel2.enabled = false;
                }
            }
            0xFF18 => {
                // NR23 - Frequency low
                self.channel2.frequency = (self.channel2.frequency & 0x0700) | (value as u16);
            }
            0xFF19 => {
                // NR24 - Frequency high/Control
                self.channel2.frequency =
                    (self.channel2.frequency & 0x00FF) | (((value & 0x07) as u16) << 8);
                self.channel2.length_timer.enabled = (value & 0x40) != 0;
                if (value & 0x80) != 0 {
                    self.channel2.trigger();
                }
            }

            // Channel 3 registers
            0xFF1A => {
                // NR30 - DAC
                self.channel3.dac_enabled = (value & 0x80) != 0;
                if !self.channel3.dac_enabled {
                    self.channel3.enabled = false;
                }
            }
            0xFF1B => {
                // NR31 - Length
                self.channel3.length_timer.length = 256 - value as u16;
            }
            0xFF1C => {
                // NR32 - Volume
                self.channel3.volume = (value >> 5) & 0x03;
            }
            0xFF1D => {
                // NR33 - Frequency low
                self.channel3.frequency = (self.channel3.frequency & 0x0700) | (value as u16);
            }
            0xFF1E => {
                // NR34 - Frequency high/Control
                self.channel3.frequency =
                    (self.channel3.frequency & 0x00FF) | (((value & 0x07) as u16) << 8);
                self.channel3.length_timer.enabled = (value & 0x40) != 0;
                if (value & 0x80) != 0 {
                    self.channel3.trigger();
                }
            }

            // Channel 4 registers
            0xFF20 => {
                // NR41 - Length
                self.channel4.length_timer.length = (64 - (value & 0x3F)) as u16;
            }
            0xFF21 => {
                // NR42 - Envelope
                self.channel4.envelope.initial_volume = (value >> 4) & 0x0F;
                self.channel4.envelope.direction = (value & 0x08) != 0;
                self.channel4.envelope.step_length = value & 0x07;
                self.channel4.dac_enabled = (value & 0xF8) != 0;
                if !self.channel4.dac_enabled {
                    self.channel4.enabled = false;
                }
            }
            0xFF22 => {
                // NR43 - Frequency/Randomness
                self.channel4.clock_shift = (value >> 4) & 0x0F;
                self.channel4.width_mode = (value & 0x08) != 0;
                self.channel4.divisor_code = value & 0x07;
            }
            0xFF23 => {
                // NR44 - Control
                self.channel4.length_timer.enabled = (value & 0x40) != 0;
                if (value & 0x80) != 0 {
                    self.channel4.trigger();
                }
            }

            // Master control registers
            0xFF24 => {
                // NR50 - Master volume
                self.left_volume = (value >> 4) & 0x07;
                self.right_volume = value & 0x07;
            }
            0xFF25 => {
                // NR51 - Sound panning
                self.left_enables = (value >> 4) & 0x0F;
                self.right_enables = value & 0x0F;
            }
            0xFF26 => {
                // NR52 - Master control
                let was_enabled = self.master_enable;
                self.master_enable = (value & 0x80) != 0;

                if was_enabled && !self.master_enable {
                    // Clear all audio registers when disabled
                    self.reset_all_channels();
                }
            }

            // Wave RAM
            0xFF30..=0xFF3F => {
                self.channel3.wave_ram[(address - 0xFF30) as usize] = value;
            }

            _ => {}
        }
    }

    fn reset_all_channels(&mut self) {
        self.channel1 = SquareChannel::new(true);
        self.channel2 = SquareChannel::new(false);
        self.channel3 = WaveChannel::new();
        self.channel4 = NoiseChannel::new();
        self.left_volume = 0;
        self.right_volume = 0;
        self.left_enables = 0;
        self.right_enables = 0;
    }
}
