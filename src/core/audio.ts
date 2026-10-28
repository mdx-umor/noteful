// Audio recording (MediaRecorder) and storage of recordings.

import type { Id } from "./model";
import { readBytes, writeBytes, deleteRaw } from "./storage";

export interface Recording { id: Id; title: string; duration: number; created: number; mime: string }

export const saveAudio = (id: Id, bytes: Uint8Array) => writeBytes(`audio/${id}`, bytes);
export const loadAudio = (id: Id) => readBytes(`audio/${id}`);
export const deleteAudio = (id: Id) => deleteRaw(`audio/${id}`);

/** Records from the microphone; time() is seconds of audio recorded so far (pauses excluded). */
export class Recorder {
  private rec: MediaRecorder | null = null;
  private stream: MediaStream | null = null;
  private chunks: Blob[] = [];
  private startedAt = 0;
  private acc = 0;
  paused = false;
  mime = "";

  async start() {
    if (!navigator.mediaDevices?.getUserMedia || typeof MediaRecorder === "undefined") throw new Error("This browser can't record audio");
    this.stream = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true } });
    const types = ["audio/mp4", "audio/webm;codecs=opus", "audio/webm", "audio/ogg;codecs=opus"];
    const type = types.find((t) => MediaRecorder.isTypeSupported(t)) ?? "";
    this.rec = new MediaRecorder(this.stream, { ...(type ? { mimeType: type } : {}), audioBitsPerSecond: 48000 });
    this.mime = this.rec.mimeType || type || "audio/mp4";
    this.chunks = [];
    this.rec.ondataavailable = (e) => { if (e.data.size) this.chunks.push(e.data); };
    this.rec.start(1000);
    this.startedAt = performance.now();
    this.acc = 0;
    this.paused = false;
  }

  time(): number {
    return this.acc + (this.paused ? 0 : (performance.now() - this.startedAt) / 1000);
  }

  pause() {
    if (!this.rec || this.paused) return;
    this.rec.pause();
    this.acc = this.time();
    this.paused = true;
  }
  resume() {
    if (!this.rec || !this.paused) return;
    this.rec.resume();
    this.startedAt = performance.now();
    this.paused = false;
  }

  /** Stops and returns the recorded bytes and duration in seconds. */
  stop(): Promise<{ bytes: Uint8Array; duration: number; mime: string }> {
    return new Promise((resolve, reject) => {
      const rec = this.rec;
      if (!rec) return reject(new Error("Not recording"));
      const duration = this.time();
      rec.onstop = async () => {
        this.stream?.getTracks().forEach((t) => t.stop());
        const blob = new Blob(this.chunks, { type: this.mime });
        resolve({ bytes: new Uint8Array(await blob.arrayBuffer()), duration, mime: this.mime });
      };
      rec.stop();
      this.rec = null;
    });
  }
}

export function fmtTime(s: number) {
  s = Math.max(0, Math.floor(s));
  const h = Math.floor(s / 3600), m = Math.floor(s / 60) % 60, sec = s % 60;
  return (h ? `${h}:${String(m).padStart(2, "0")}` : String(m)) + ":" + String(sec).padStart(2, "0");
}
