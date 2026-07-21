use gstreamer as gst;
use gstreamer::glib;
use gstreamer::prelude::*;

/// Thin wrapper around a GStreamer `playbin`.
///
/// `playbin` auto-plugs decoders for whatever format the file is, and gives us
/// gapless-friendly playback, seeking and volume for free. `Element` is a
/// ref-counted GObject, so cloning a `Player` is cheap and shares one pipeline.
#[derive(Clone)]
pub struct Player {
    playbin: gst::Element,
}

impl Player {
    pub fn new() -> Self {
        let playbin = gst::ElementFactory::make("playbin")
            .build()
            .expect("the `playbin` element should be available");

        // Give playbin an explicit audio chain. Letting autoaudiosink negotiate
        // the raw stream format directly can produce gritty, distorted output;
        // inserting audioconvert + audioresample guarantees a clean conversion.
        if let Ok(sink) =
            gst::parse::bin_from_description("audioconvert ! audioresample ! autoaudiosink", true)
        {
            playbin.set_property("audio-sink", &sink);
        }

        Self { playbin }
    }

    /// Switch to a new file and start it. Expects a URI, e.g.
    /// `file:///music/song.flac`.
    ///
    /// `playbin` only accepts a new `uri` from the `Ready`/`Null` state, so we
    /// drop the pipeline back to `Ready` first. Skipping this leaves the old
    /// track playing and can overlap the two streams into garbled audio.
    pub fn play_uri(&self, uri: &str) {
        let _ = self.playbin.set_state(gst::State::Ready);
        self.playbin.set_property("uri", uri);
        let _ = self.playbin.set_state(gst::State::Playing);
    }

    pub fn play(&self) {
        let _ = self.playbin.set_state(gst::State::Playing);
    }

    pub fn pause(&self) {
        let _ = self.playbin.set_state(gst::State::Paused);
    }

    /// 0.0 = muted, 1.0 = unity gain.
    pub fn set_volume(&self, volume: f64) {
        self.playbin.set_property("volume", volume.clamp(0.0, 1.0));
    }

    /// Current playback position, if the pipeline can report it yet.
    pub fn position(&self) -> Option<gst::ClockTime> {
        self.playbin.query_position::<gst::ClockTime>()
    }

    /// Total length of the current stream, once known.
    pub fn duration(&self) -> Option<gst::ClockTime> {
        self.playbin.query_duration::<gst::ClockTime>()
    }

    /// Jump to `seconds` into the current track.
    pub fn seek(&self, seconds: f64) {
        let target = gst::ClockTime::from_seconds(seconds.max(0.0) as u64);
        let _ = self.playbin.seek_simple(
            gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
            target,
        );
    }

    /// Run `f` on the main thread each time the current track finishes.
    pub fn connect_eos<F: Fn() + 'static>(&self, f: F) {
        if let Some(bus) = self.playbin.bus() {
            let _ = bus.add_watch_local(move |_, msg| {
                if let gst::MessageView::Eos(_) = msg.view() {
                    f();
                }
                glib::ControlFlow::Continue
            });
        }
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}
