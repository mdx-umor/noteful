import { useEffect, useRef, useState } from "react";
import type { Id, RecordingMeta } from "../core/model";
import { newId } from "../core/model";
import { deleteAudio, fmtTime, loadAudio, Recorder, saveAudio } from "../core/audio";
import { download } from "../core/pdf";
import { saveDoc } from "../core/storage";
import { Icon } from "../ui/icons";
import { Popover, type Anchor } from "../ui/ui";
import type { DocModel } from "./doc";

interface Playing { rec: RecordingMeta; audio: HTMLAudioElement; url: string }

/** Recordings popover plus the floating recorder / player bars. */
export function AudioLayer({ model, toast, panelAt, onClosePanel, onState }: {
  model: DocModel; toast: (s: string) => void; panelAt: Anchor | null; onClosePanel: () => void; onState: (recording: boolean) => void;
}) {
  const recorder = useRef<Recorder | null>(null);
  const recId = useRef<Id>(0);
  const [recording, setRecording] = useState(false);
  const [paused, setPaused] = useState(false);
  const [playing, setPlaying] = useState<Playing | null>(null);
  const [, tick] = useState(0);
  const [renaming, setRenaming] = useState<{ id: Id; title: string } | null>(null);

  useEffect(() => { onState(recording); }, [recording, onState]);
  useEffect(() => {
    if (!recording && !playing) return;
    const t = setInterval(() => tick((x) => x + 1), 250);
    return () => clearInterval(t);
  }, [recording, playing]);

  // Tapping ink written during the playing recording jumps there.
  useEffect(() => {
    if (!playing) { model.onInkTap = null; return; }
    model.onInkTap = (it) => {
      const a = "Stroke" in it.content ? it.content.Stroke.audio : undefined;
      if (!a || a.rec !== playing.rec.id) return false;
      playing.audio.currentTime = Math.max(0, a.t - 1);
      playing.audio.play().catch(() => {});
      return true;
    };
    return () => { model.onInkTap = null; };
  }, [playing, model]);

  // Stop everything when the notebook closes.
  useEffect(() => () => {
    model.audioClock = null;
    if (recorder.current) void finish();
    stopPlaying();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function start() {
    stopPlaying();
    try {
      const r = new Recorder();
      await r.start();
      recorder.current = r;
      recId.current = newId();
      model.audioClock = () => ({ rec: recId.current, t: r.time() });
      setRecording(true);
      setPaused(false);
      onClosePanel();
      toast("Recording. Your writing is linked to the audio.");
    } catch (e) {
      toast("Could not start recording: " + (e instanceof Error ? e.message : String(e)));
    }
  }

  async function finish() {
    const r = recorder.current;
    if (!r) return;
    recorder.current = null;
    model.audioClock = null;
    setRecording(false);
    try {
      const { bytes, duration, mime } = await r.stop();
      saveAudio(recId.current, bytes);
      const n = model.recordings.length + 1;
      model.setRecordings((list) => [...list, { id: recId.current, title: `Recording ${n}`, duration, created: Date.now(), mime }]);
      saveDoc(model.doc); // also covers stopping because the notebook was closed
      toast(`Saved recording (${fmtTime(duration)})`);
    } catch (e) {
      toast("Recording failed: " + (e instanceof Error ? e.message : String(e)));
    }
  }

  function play(rec: RecordingMeta) {
    stopPlaying();
    const bytes = loadAudio(rec.id);
    if (!bytes) return toast("Recording not found");
    const url = URL.createObjectURL(new Blob([bytes], { type: rec.mime }));
    const audio = new Audio(url);
    audio.onended = () => tick((x) => x + 1);
    audio.play().catch(() => {});
    setPlaying({ rec, audio, url });
    onClosePanel();
    toast("Tap your writing to jump to when it was written");
  }

  function stopPlaying() {
    setPlaying((p) => {
      if (p) { p.audio.pause(); URL.revokeObjectURL(p.url); }
      return null;
    });
  }

  const recs = model.recordings;
  return (
    <>
      {panelAt && (
        <Popover anchor={panelAt} onClose={onClosePanel} width={380}>
          <h3>Recordings</h3>
          {recording ? (
            <button className="btn primary" style={{ width: "100%" }} onClick={finish}>Stop recording</button>
          ) : (
            <button className="btn primary" style={{ width: "100%" }} onClick={start}><span className="row" style={{ justifyContent: "center" }}><span style={{ width: 10, height: 10, borderRadius: 5, background: "#ff3b30" }} />Record</span></button>
          )}
          <div className="menu" style={{ marginTop: 10 }}>
            {recs.map((r) => (
              <div key={r.id} className="row" style={{ padding: "6px 4px" }}>
                <button className="icon-btn" title="Play" onClick={() => play(r)}><Icon name="chevronRight" /></button>
                <div className="grow" style={{ minWidth: 0 }}>
                  <div style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{r.title}</div>
                  <div className="muted small">{fmtTime(r.duration)} · {new Date(r.created).toLocaleDateString()}</div>
                </div>
                <button className="icon-btn" title="Rename" onClick={() => setRenaming({ id: r.id, title: r.title })}><Icon name="text" size={18} /></button>
                <button className="icon-btn" title="Download" onClick={() => { const b = loadAudio(r.id); if (b) download(`${r.title}.${r.mime.includes("mp4") ? "m4a" : r.mime.includes("ogg") ? "ogg" : "webm"}`, b, r.mime); }}><Icon name="download" size={18} /></button>
                <button className="icon-btn" title="Delete" onClick={() => {
                  if (!confirm(`Delete "${r.title}"?`)) return;
                  if (playing?.rec.id === r.id) stopPlaying();
                  deleteAudio(r.id);
                  model.setRecordings((l) => l.filter((x) => x.id !== r.id));
                }}><Icon name="trash" size={18} /></button>
              </div>
            ))}
          </div>
          {!recs.length && <p className="muted small">Record a lecture while you take notes. Later, play it back and tap your writing to hear what was said at that moment.</p>}
          {renaming && (
            <div className="row" style={{ marginTop: 8 }}>
              <input type="text" autoFocus value={renaming.title} onChange={(e) => setRenaming({ ...renaming, title: e.target.value })}
                onKeyDown={(e) => { if (e.key === "Enter") { model.setRecordings((l) => l.map((x) => (x.id === renaming.id ? { ...x, title: renaming.title.trim() || x.title } : x))); setRenaming(null); } }} />
              <button className="btn" onClick={() => { model.setRecordings((l) => l.map((x) => (x.id === renaming.id ? { ...x, title: renaming.title.trim() || x.title } : x))); setRenaming(null); }}>Save</button>
            </div>
          )}
        </Popover>
      )}
      {recording && recorder.current && (
        <div className="floating sel-menu" style={{ left: "50%", top: 12, transform: "translateX(-50%)" }}>
          <span style={{ width: 10, height: 10, borderRadius: 5, background: paused ? "var(--muted)" : "#ff3b30", margin: "0 8px" }} />
          <span style={{ fontVariantNumeric: "tabular-nums", minWidth: 48 }}>{fmtTime(recorder.current.time())}</span>
          <button className="btn" onClick={() => { if (paused) recorder.current?.resume(); else recorder.current?.pause(); setPaused(!paused); }}>{paused ? "Resume" : "Pause"}</button>
          <button className="btn primary" onClick={finish}>Stop</button>
        </div>
      )}
      {playing && (
        <div className="floating sel-menu" style={{ left: "50%", top: 12, transform: "translateX(-50%)", padding: "5px 10px" }}>
          <button className="btn" onClick={() => { if (playing.audio.paused) playing.audio.play(); else playing.audio.pause(); tick((x) => x + 1); }}>{playing.audio.paused ? "Play" : "Pause"}</button>
          <span className="small" style={{ fontVariantNumeric: "tabular-nums" }}>{fmtTime(playing.audio.currentTime)}</span>
          <input type="range" min={0} max={playing.rec.duration || playing.audio.duration || 1} step={0.1} value={playing.audio.currentTime}
            onChange={(e) => { playing.audio.currentTime = Number(e.target.value); tick((x) => x + 1); }} style={{ width: 180 }} />
          <span className="small muted" style={{ fontVariantNumeric: "tabular-nums" }}>{fmtTime(playing.rec.duration)}</span>
          <button className="icon-btn" title="Close" onClick={stopPlaying}><Icon name="close" size={18} /></button>
        </div>
      )}
    </>
  );
}
