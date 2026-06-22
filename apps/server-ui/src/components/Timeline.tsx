import { getAudioBlob } from '@/api';
import type { RoundData } from '@/data/round-data';
import { ActionIcon, Box, Group, Stack, Text, Badge } from '@mantine/core';
import { IconPlayerPlayFilled, IconPlayerPauseFilled, IconPlayerStop, IconZoomIn, IconZoomOut } from '@tabler/icons-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import WaveSurfer from 'wavesurfer.js';
import RegionsPlugin from 'wavesurfer.js/dist/plugins/regions';
import TimelinePlugin from 'wavesurfer.js/dist/plugins/timeline';

interface TimelineProps {
  roundId: string;
  dataItems: RoundData[];
}

const stepLabels: Record<string, string> = {
  input_audio: '语音输入',
  asr: '语音识别',
  text: '文本输入',
  llm: '大模型',
  tts: '语音合成',
};

const stepColors: Record<string, string> = {
  input_audio: '#40c057',
  asr: '#15aabf',
  text: '#fab005',
  llm: '#7950f2',
  tts: '#fa5252',
};

function formatBadgeText(data: RoundData): string {
  const label = stepLabels[data.data_type] ?? data.data_type;
  const meta = data.metadata;
  const audioDurMs = meta?.audio_duration_ms as number | undefined;
  const procMs = meta?.duration_ms as number | undefined;

  if (data.data_type === 'input_audio') {
    const dur = audioDurMs != null ? ` ${(audioDurMs / 1000).toFixed(1)}s` : '';
    return `${label}${dur}`;
  }

  if (data.data_type === 'tts') {
    const dur = audioDurMs != null ? ` ${(audioDurMs / 1000).toFixed(1)}s` : '';
    const proc = procMs != null ? ` ⏱${(procMs / 1000).toFixed(procMs < 1000 ? 1 : 0)}s` : '';
    return `${label}${dur}${proc}`;
  }

  if (data.text) {
    const txt = data.text.length <= 10
      ? ` "${data.text}"`
      : ` "${data.text.slice(0, 10)}..."(${data.text.length}字)`;
    const proc = procMs != null ? ` ⏱${(procMs / 1000).toFixed(procMs < 1000 ? 1 : 0)}s` : '';
    return `${label}${txt}${proc}`;
  }

  return label;
}

interface ClipData {
  id: string;
  dataType: string;
  color: string;
  startMs: number;
  endMs: number;
  durationMs: number;
  label: string;
}

function writeStr(view: DataView, off: number, s: string) {
  for (let i = 0; i < s.length; i++) view.setUint8(off + i, s.charCodeAt(i));
}

function audioBufferToWavBlob(buf: AudioBuffer): Blob {
  const numCh = buf.numberOfChannels;
  const sr = buf.sampleRate;
  const bits = 16;
  const bytesPer = bits / 8;
  const blockAlign = numCh * bytesPer;
  const dataLen = buf.length * blockAlign;
  const totalLen = 44 + dataLen;
  const ab = new ArrayBuffer(totalLen);
  const v = new DataView(ab);
  writeStr(v, 0, 'RIFF');
  v.setUint32(4, totalLen - 8, true);
  writeStr(v, 8, 'WAVE');
  writeStr(v, 12, 'fmt ');
  v.setUint32(16, 16, true);
  v.setUint16(20, 1, true);
  v.setUint16(22, numCh, true);
  v.setUint32(24, sr, true);
  v.setUint32(28, sr * blockAlign, true);
  v.setUint16(32, blockAlign, true);
  v.setUint16(34, bits, true);
  writeStr(v, 36, 'data');
  v.setUint32(40, dataLen, true);
  let off = 44;
  const chs: Float32Array[] = [];
  for (let c = 0; c < numCh; c++) chs.push(buf.getChannelData(c));
  for (let i = 0; i < buf.length; i++) {
    for (let c = 0; c < numCh; c++) {
      const s = Math.max(-1, Math.min(1, chs[c][i]));
      v.setInt16(off, s < 0 ? s * 0x8000 : s * 0x7FFF, true);
      off += 2;
    }
  }
  return new Blob([ab], { type: 'audio/wav' });
}

export function Timeline({ roundId, dataItems }: TimelineProps) {
  const { t } = useTranslation();
  const containerRef = useRef<HTMLDivElement>(null);
  const wsRef = useRef<WaveSurfer | null>(null);
  const blobUrlRef = useRef<string | null>(null);

  const [isPlaying, setIsPlaying] = useState(false);
  const [isReady, setIsReady] = useState(false);
  const [pixelsPerSecond, setPixelsPerSecond] = useState(80);

  const sorted = useMemo(() => {
    return [...dataItems]
      .filter((d) => d.metadata?.duration_ms != null)
      .sort((a, b) => ((a.metadata?.duration_ms as number) ?? 0) - ((b.metadata?.duration_ms as number) ?? 0));
  }, [dataItems]);

  const t0Ms = useMemo(() => {
    let min = Infinity;
    for (const d of sorted) min = Math.min(min, (d.metadata?.duration_ms as number) ?? 0);
    return isFinite(min) ? min : 0;
  }, [sorted]);

  const clips = useMemo(() => {
    const result: ClipData[] = [];
    for (let i = 0; i < sorted.length; i++) {
      const d = sorted[i];
      const dt = (d.metadata?.duration_ms as number) ?? 0;
      let durMs: number;
      if (d.data_type === 'input_audio' || d.data_type === 'tts') {
        durMs = (d.metadata?.audio_duration_ms as number) ?? 500;
      } else {
        const next = sorted[i + 1];
        if (next) {
          durMs = ((next.metadata?.duration_ms as number) ?? 0) - dt;
        } else {
          durMs = 300;
        }
      }
      durMs = Math.max(durMs, 10);
      const startMs = dt - t0Ms;
      result.push({
        id: d.id,
        dataType: d.data_type,
        color: stepColors[d.data_type] ?? '#868e96',
        startMs,
        endMs: startMs + durMs,
        durationMs: durMs,
        label: stepLabels[d.data_type] ?? d.data_type,
      });
    }
    return result;
  }, [sorted, t0Ms]);

  const totalDurationMs = useMemo(() => {
    if (clips.length === 0) return 1000;
    return Math.max(...clips.map((c) => c.endMs), 1000);
  }, [clips]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    container.innerHTML = '';
    let cancelled = false;

    (async () => {
      const audioSteps = sorted.filter(
        (d) => (d.data_type === 'input_audio' || d.data_type === 'tts') && d.create_datetime,
      );
      const audioMap = new Map<string, AudioBuffer>();

      for (const d of audioSteps) {
        try {
          const blob = await getAudioBlob(d.round_id, d.id);
          const arrayBuf = await blob.arrayBuffer();
          const dec = new AudioContext();
          const ab = await dec.decodeAudioData(arrayBuf);
          await dec.close();
          audioMap.set(d.id, ab);
        } catch {
          // skip failed clips
        }
      }

      if (cancelled) return;

      const sampleRate = 44100;
      const totalSamples = Math.ceil((totalDurationMs / 1000) * sampleRate);

      let combined: AudioBuffer;
      if (totalSamples < 1) return;

      const offline = new OfflineAudioContext(1, totalSamples, sampleRate);
      for (const clip of clips) {
        const buf = audioMap.get(clip.id);
        if (!buf) continue;
        const src = offline.createBufferSource();
        src.buffer = buf;
        src.connect(offline.destination);
        src.start(clip.startMs / 1000);
      }

      try {
        combined = await offline.startRendering();
      } catch {
        if (cancelled) return;
        combined = offline.createBuffer(1, totalSamples, sampleRate);
      }

      if (cancelled) return;

      const wavBlob = audioBufferToWavBlob(combined);

      const regions = RegionsPlugin.create();
      const timeline = TimelinePlugin.create({
        height: 20,
        formatTimeCallback: (sec: number) => `${sec.toFixed(0)}s`,
      });

      const ws = WaveSurfer.create({
        container,
        waveColor: '#dee2e6',
        progressColor: '#4a9eff',
        fillParent: true,
        minPxPerSec: 10,
        autoScroll: true,
        autoCenter: false,
        plugins: [regions, timeline],
        backend: 'WebAudio',
      });

      await ws.loadBlob(wavBlob);

      if (cancelled) { ws.destroy(); return; }

      for (const clip of clips) {
        const el = document.createElement('span');
        el.textContent = clip.label;
        el.style.cssText = 'font-size:10px;color:#fff;font-weight:600;line-height:1.4';
        regions.addRegion({
          start: clip.startMs / 1000,
          end: clip.endMs / 1000,
          color: clip.color + '30',
          content: el,
          drag: false,
          resize: false,
        });
      }

      ws.zoom(pixelsPerSecond);

      ws.on('play', () => setIsPlaying(true));
      ws.on('pause', () => setIsPlaying(false));
      ws.on('finish', () => setIsPlaying(false));

      wsRef.current = ws;
      setIsReady(true);
    })();

    return () => {
      cancelled = true;
      wsRef.current?.destroy();
      wsRef.current = null;
      if (blobUrlRef.current) {
        URL.revokeObjectURL(blobUrlRef.current);
        blobUrlRef.current = null;
      }
      setIsReady(false);
      setIsPlaying(false);
    };
  }, [roundId]);

  useEffect(() => {
    wsRef.current?.zoom(pixelsPerSecond);
  }, [pixelsPerSecond]);

  const handlePlay = () => wsRef.current?.play();
  const handlePause = () => wsRef.current?.pause();
  const handleStop = () => wsRef.current?.stop();

  return (
    <Stack gap={4}>
      <Group justify="flex-end" gap={4}>
        <ActionIcon variant="subtle" color="gray" size="sm" onClick={() => setPixelsPerSecond((z) => Math.max(z / 1.5, 10))}>
          <IconZoomOut />
        </ActionIcon>
        <ActionIcon variant="subtle" color="gray" size="sm" onClick={() => setPixelsPerSecond((z) => Math.min(z * 1.5, 1000))}>
          <IconZoomIn />
        </ActionIcon>
        <ActionIcon variant="subtle" color="gray" size="sm" onClick={handlePlay} disabled={isPlaying}>
          <IconPlayerPlayFilled />
        </ActionIcon>
        <ActionIcon variant="subtle" color="gray" size="sm" onClick={handlePause} disabled={!isPlaying}>
          <IconPlayerPauseFilled />
        </ActionIcon>
        <ActionIcon variant="subtle" color="gray" size="sm" onClick={handleStop} disabled={!isPlaying}>
          <IconPlayerStop />
        </ActionIcon>
      </Group>

      {sorted.length > 0 && (
        <Group gap={4} align="center" wrap="wrap">
          {sorted.map((d, i) => (
            <Group key={d.id} gap={2} wrap="nowrap">
              {i > 0 && <Text c="dimmed" size="xs">→</Text>}
              <Badge
                color={stepColors[d.data_type] ?? 'gray'}
                variant="light"
                size="sm"
                style={{ textTransform: 'none' }}
              >
                {formatBadgeText(d)}
              </Badge>
            </Group>
          ))}
        </Group>
      )}

      <Box
        ref={containerRef}
        style={{
          minHeight: 80,
          borderRadius: 4,
          overflow: 'hidden',
        }}
      />

      {!isReady && sorted.length > 0 && (
        <Text size="sm" c="dimmed" ta="center" py="sm">
          {t('loading')}
        </Text>
      )}
    </Stack>
  );
}
