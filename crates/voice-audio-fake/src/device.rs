//! Okres urządzenia atrapy (10 ms): render wyjść, historia do echa, mikrofon z echem „pokoju”,
//! zapis do strumieni wejściowych; sprawdzanie otwarcia urządzenia.

use voice_audio_contract::{AudioError, DeviceId, DeviceKind, MediaTime};

use crate::{FAKE_RATE, HISTORY, Inner, PERIOD, PERIOD_SAMPLES};

impl Inner {
    pub(crate) fn step(&mut self) {
        let t = self.now;
        let play_ts = t.plus(self.output_latency);
        let mut mix = vec![0.0f32; PERIOD_SAMPLES];
        for o in &mut self.outputs {
            let ch = usize::from(o.channels);
            let mut buf = vec![0.0f32; PERIOD_SAMPLES * ch];
            o.render.render(&mut buf, o.channels, play_ts);
            for (m, f) in mix.iter_mut().zip(buf.chunks_exact(ch)) {
                *m += f[0];
            }
        }
        self.recorded.extend_from_slice(&mix);
        // Historia „odtworzonego” dźwięku: próbka renderowana w chwili t gra w t + opóźnienie.
        self.history.extend(mix.iter().copied());
        while self.history.len() > HISTORY {
            self.history.pop_front();
            self.history_start += 1;
        }
        let rendered_start = t.to_samples(FAKE_RATE);
        let lat = MediaTime(self.output_latency.as_nanos() as u64).to_samples(FAKE_RATE);
        let mut mic = vec![0.0f32; PERIOD_SAMPLES];
        for (i, m) in mic.iter_mut().enumerate() {
            if self.mic_pos < self.mic.len() {
                *m = self.mic[self.mic_pos];
                self.mic_pos += 1;
                if self.mic_loop && self.mic_pos == self.mic.len() {
                    self.mic_pos = 0;
                }
            }
            if let Some(echo) = &self.echo {
                let now = rendered_start + i as u64;
                let delay = echo.delay_samples(FAKE_RATE) as u64 + lat;
                let mut acc = 0.0f32;
                for &(k, tap) in &self.echo_taps {
                    let Some(src) = now.checked_sub(delay + k as u64) else {
                        break;
                    };
                    if src < self.history_start {
                        break;
                    }
                    if let Some(v) = self.history.get((src - self.history_start) as usize) {
                        acc += tap * v;
                    }
                }
                *m += acc;
            }
        }
        self.inputs.retain(|i| !i.writer.is_abandoned());
        for input in &mut self.inputs {
            let src = if input.loopback { &mix } else { &mic };
            let ch = usize::from(input.channels);
            let block: Vec<f32> = src
                .iter()
                .flat_map(|&s| std::iter::repeat_n(s, ch))
                .collect();
            let ts = if input.loopback { play_ts } else { t };
            input.writer.write(&block, ts);
        }
        self.now = t.plus(PERIOD);
    }

    pub(crate) fn check_open(
        &mut self,
        device: Option<&DeviceId>,
        kind: DeviceKind,
    ) -> Result<DeviceId, AudioError> {
        if let Some(err) = self.fail_next_open.take() {
            return Err(err);
        }
        let found = self
            .devices
            .iter()
            .find(|d| d.kind == kind && device.map_or(d.is_default, |id| &d.id == id));
        match (found, device) {
            (Some(d), _) => Ok(d.id.clone()),
            (None, Some(id)) => Err(AudioError::DeviceNotFound(id.to_string())),
            (None, None) => Err(AudioError::NoDefaultDevice),
        }
    }
}
