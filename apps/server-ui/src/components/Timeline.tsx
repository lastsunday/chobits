import { getAudioBlob, listFrames } from '@/api';
import type { RoundData } from '@/data/round-data';
import { ActionIcon, Box, Group, Stack, Switch, Text, Tooltip } from '@mantine/core';
import { IconChevronDown, IconChevronRight, IconPlayerPlayFilled, IconPlayerPauseFilled, IconPlayerStop, IconZoomIn, IconZoomOut } from '@tabler/icons-react';
import { useQuery } from '@tanstack/react-query';
import { memo, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
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

function parseFrameLabel(detail: string | null): string {
  if (!detail) return '?';
  const prefix = detail.split(/[\(\{]/)[0].trim();
  const map: Record<string, string> = {
    Hello: 'hello',
    HelloResult: 'hello',
    Listen: 'listen',
    Voice: 'voice',
    UnknowText: 'text',
    STTResult: 'asr',
    LLMResult: 'llm',
    TTSResult: 'tts',
    AudioResult: 'audio',
    Abort: 'abort',
    CloseResult: 'close',
    McpResult: 'mcp',
    Mcp: 'mcp',
    Error: 'error',
  };
  return map[prefix] ?? prefix.slice(0, 5).toLowerCase();
}

const frameTypeColors: Record<string, string> = {
  hello: '#20c997',
  listen: '#868e96',
  voice: '#40c057',
  text: '#fab005',
  asr: '#15aabf',
  llm: '#7950f2',
  tts: '#fa5252',
  audio: '#4dabf7',
  abort: '#e03131',
  close: '#e64980',
  mcp: '#0ca678',
  error: '#e03131',
};

function lightenColor(hex: string, amount: number): string {
  const num = parseInt(hex.slice(1), 16);
  const r = Math.min(255, (num >> 16) + Math.round(255 * amount));
  const g = Math.min(255, ((num >> 8) & 0xff) + Math.round(255 * amount));
  const b = Math.min(255, (num & 0xff) + Math.round(255 * amount));
  return `#${((r << 16) | (g << 8) | b).toString(16).padStart(6, '0')}`;
}

const FrameGridCell = memo(function FrameGridCell({
  seq,
  dir,
  seekMs,
  color,
  isActive,
  detail,
  kind,
  onSeek,
}: {
  seq: number;
  dir: string;
  seekMs: number;
  color: string;
  isActive: boolean;
  detail: string | null;
  kind: string;
  onSeek: (seekMs: number) => void;
}) {
  const [phase, setPhase] = useState<'none' | 'sweeping' | 'active' | 'fading'>('none');
  const wasActiveRef = useRef(false);

  useLayoutEffect(() => {
    const was = wasActiveRef.current;
    wasActiveRef.current = isActive;
    if (isActive && !was) {
      setPhase('sweeping');
    } else if (!isActive && was) {
      setPhase('fading');
    }
  }, [isActive]);

  const handleAnimationEnd = useCallback(() => {
    setPhase((prev) => (prev === 'sweeping' ? 'active' : prev));
  }, []);

  useEffect(() => {
    if (phase !== 'fading') return;
    const timer = setTimeout(() => setPhase('none'), 800);
    return () => clearTimeout(timer);
  }, [phase]);

  const bgInactive = lightenColor(color, 0.6);
  const textInactive = '#495057';
  const bgActiveBase = color;
  const bgActiveHighlight = lightenColor(color, 0.4);
  const dirColor = dir === 'inbound' ? '#228be6' : '#fab005';

  const isHighlighted = phase === 'sweeping' || phase === 'active';

  return (
    <Tooltip
      label={
        <Box>
          <Text size="xs" fw={600}>#{seq}</Text>
          <Text size="xs">{dir === 'inbound' ? '\u2193 inbound' : '\u2191 outbound'}</Text>
          <Text size="xs">{kind}</Text>
          {detail && <Text size="xs" style={{ maxWidth: 240, wordBreak: 'break-all' }}>{detail}</Text>}
        </Box>
      }
      withArrow
      openDelay={300}
    >
      <Box
        data-frame-seq={seq}
        onClick={() => onSeek(seekMs)}
        onAnimationEnd={handleAnimationEnd}
        style={{
          width: 36,
          height: 28,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          borderRadius: 4,
          cursor: 'pointer',
          fontFamily: 'monospace',
          fontSize: 11,
          fontWeight: 400,
          userSelect: 'none',
          position: 'relative',
          background:
            phase === 'sweeping'
              ? `linear-gradient(90deg, ${bgActiveBase}, ${bgActiveHighlight}, ${bgActiveBase})`
              : phase === 'active'
                ? bgActiveBase
                : bgInactive,
          backgroundSize: phase === 'sweeping' ? '200% 100%' : undefined,
          animation: phase === 'sweeping' ? 'frameCellSweep 0.5s ease-in-out 1 forwards' : undefined,
          color: isHighlighted ? '#fff' : textInactive,
          transform: isHighlighted ? 'scale(1.35)' : 'scale(1)',
          boxShadow: isHighlighted ? `0 0 6px ${color}` : 'none',
          transition:
            phase === 'fading'
              ? 'background 0.8s, transform 0.8s, box-shadow 0.8s, color 0.8s'
              : 'transform 0.2s, box-shadow 0.2s, color 0.2s',
          zIndex: isHighlighted ? 1 : 0,
          borderTop: dir === 'outbound' ? `2px solid ${dirColor}` : 'none',
          borderBottom: dir === 'inbound' ? `2px solid ${dirColor}` : 'none',
        }}
      >
        {seq}
      </Box>
    </Tooltip>
  );
});

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
  const regionsRef = useRef<RegionsPlugin | null>(null);
  const [isPlaying, setIsPlaying] = useState(false);
  const [isReady, setIsReady] = useState(false);
  const [showFrames, setShowFrames] = useState(false);
  const [pixelsPerSecond, setPixelsPerSecond] = useState(80);
  const [currentTime, setCurrentTime] = useState(0);
  const [syncMode, setSyncMode] = useState(true);

  const sorted = useMemo(() => {
    return [...dataItems]
      .filter((d) => {
        const pos = (d.metadata?.elapsed_ms as number) ?? (d.metadata?.duration_ms as number);
        return pos != null;
      })
      .sort((a, b) => {
        const pa = (a.metadata?.elapsed_ms as number) ?? (a.metadata?.duration_ms as number) ?? 0;
        const pb = (b.metadata?.elapsed_ms as number) ?? (b.metadata?.duration_ms as number) ?? 0;
        return pa - pb;
      });
  }, [dataItems]);

  const t0Ms = useMemo(() => {
    let min = Infinity;
    for (const d of sorted) {
      const pos = (d.metadata?.elapsed_ms as number) ?? (d.metadata?.duration_ms as number) ?? 0;
      min = Math.min(min, pos);
    }
    return isFinite(min) ? min : 0;
  }, [sorted]);

  const clips = useMemo(() => {
    const result: ClipData[] = [];
    for (let i = 0; i < sorted.length; i++) {
      const d = sorted[i];
      const dt = (d.metadata?.elapsed_ms as number) ?? (d.metadata?.duration_ms as number) ?? 0;
      let durMs: number;
      if (d.data_type === 'input_audio' || d.data_type === 'tts') {
        durMs = (d.metadata?.audio_duration_ms as number) ?? 500;
      } else {
        const next = sorted[i + 1];
        if (next) {
          const nextPos = (next.metadata?.elapsed_ms as number) ?? (next.metadata?.duration_ms as number) ?? 0;
          durMs = nextPos - dt;
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

  const { data: framesData } = useQuery({
    queryKey: ['round-frames', roundId],
    queryFn: () => listFrames(roundId),
    enabled: !!roundId,
  });

  const ttsStep = useMemo(() => {
    return sorted.find((d) => d.data_type === 'tts') ?? null;
  }, [sorted]);

  const ttsSyncPositions = useMemo(() => {
    const map = new Map<number, number>();
    if (!syncMode || !ttsStep) return map;

    const ttsElapsedMs = ttsStep.metadata?.elapsed_ms as number | undefined;
    const ttsDurationMs = ttsStep.metadata?.audio_duration_ms as number | undefined;
    const items = framesData?.items ?? [];
    if (ttsElapsedMs == null || ttsDurationMs == null || items.length === 0) return map;

    const baseMs = ttsElapsedMs - t0Ms;
    const ttsFrames = items.filter(
      (f) => f.detail?.startsWith('AudioResult') || f.detail?.startsWith('TTSResult'),
    );
    const audioFrames = ttsFrames.filter((f) => f.detail?.startsWith('AudioResult'));
    const totalAudio = audioFrames.length;

    for (const f of ttsFrames) {
      if (f.detail?.startsWith('TTSResult')) {
        if (f.detail.includes('Start') || f.detail.includes('SentenceStart')) {
          map.set(f.seq, baseMs);
        } else {
          map.set(f.seq, baseMs + ttsDurationMs);
        }
      }
    }

    audioFrames.forEach((f, i) => {
      const pos = totalAudio > 1
        ? baseMs + (i / (totalAudio - 1)) * ttsDurationMs
        : baseMs;
      map.set(f.seq, pos);
    });

    return map;
  }, [syncMode, ttsStep, t0Ms, framesData]);

  const frameList = useMemo(() => {
    if (!framesData?.items.length) return [];
    const t0 = t0Ms;
    return framesData.items.map((f) => {
      const label = parseFrameLabel(f.detail);
      const seekMs = ttsSyncPositions.get(f.seq) ?? (f.elapsed_us != null ? f.elapsed_us / 1000 - t0 : 0);
      return {
        seq: f.seq,
        dir: f.dir,
        kind: f.kind,
        detail: f.detail,
        elapsed_us: f.elapsed_us,
        seekMs,
        label,
        color: frameTypeColors[label] ?? '#868e96',
      };
    });
  }, [framesData, ttsSyncPositions, t0Ms]);

  const totalDurationMs = useMemo(() => {
    const clipMax = clips.length > 0 ? Math.max(...clips.map((c) => c.endMs), 1000) : 0;
    const frameMax = frameList.length > 0
      ? Math.max(...frameList.map((f) => f.seekMs), 0)
      : 0;
    const result = Math.max(clipMax, frameMax, 1000);
    console.log(`[Timeline:${roundId.slice(-6)}] totalDurationMs: clipMax=${clipMax}, frameMax=${frameMax}, result=${result}`);
    return result;
  }, [clips, frameList, t0Ms]);

  const frameMarkers = useMemo(() => {
    if (!framesData?.items.length) return [];

    const result: Array<{ seq: number; dir: string; kind: string; detail: string | null; startMs: number; color: string }> = [];
    let filteredCount = 0;
    let nullElapsed = 0;
    for (const f of framesData.items) {
      if (f.elapsed_us == null) { nullElapsed++; continue; }
      const startMs = ttsSyncPositions.get(f.seq) ?? ((f.elapsed_us as number) / 1000 - t0Ms);
      if (startMs < 0 || startMs > totalDurationMs) { filteredCount++; continue; }
      result.push({
        seq: f.seq,
        dir: f.dir,
        kind: f.kind,
        detail: f.detail,
        startMs,
        color: f.dir === 'inbound' ? '#228be6' : '#fab005',
      });
    }
    console.log(`[Timeline:${roundId.slice(-6)}] frameMarkers: total=${framesData.items.length}, null_elapsed=${nullElapsed}, filtered=${filteredCount}, markers=${result.length}, t0Ms=${t0Ms}, totalDurationMs=${totalDurationMs}`);
    result.sort((a, b) => a.startMs - b.startMs || a.seq - b.seq);
    if (result.length > 0) {
      console.log(`[Timeline:${roundId.slice(-6)}]  sorted: first_seq=${result[0].seq} startMs=${result[0].startMs}`, `last_seq=${result[result.length-1].seq} startMs=${result[result.length-1].startMs}`);
    }
    return result;
  }, [framesData, ttsSyncPositions, t0Ms, totalDurationMs]);

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
      regionsRef.current = regions;
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
        el.style.cssText = `font-size:10px;color:${clip.color};background:#fff;padding:0 6px;border-radius:4px;font-weight:600;line-height:1.6;margin:2px 4px;display:inline-block`;
        regions.addRegion({
          start: clip.startMs / 1000,
          end: clip.endMs / 1000,
          color: clip.color + '70',
          content: el,
          drag: false,
          resize: false,
        });
      }

      ws.zoom(pixelsPerSecond);

      ws.on('interaction', () => ws.playPause());

      ws.on('play', () => { setIsPlaying(true); setShowFrames(true); });
      ws.on('pause', () => setIsPlaying(false));
      ws.on('finish', () => { setIsPlaying(false); setCurrentTime(0); });

      ws.on('timeupdate', (time) => {
        setCurrentTime(time);
      });

      container.addEventListener('wheel', (e) => {
        e.preventDefault();
        setPixelsPerSecond((z) => {
          const factor = e.deltaY < 0 ? 1.3 : 1 / 1.3;
          return Math.max(10, Math.min(1000, z * factor));
        });
      }, { passive: false });

      wsRef.current = ws;
      setIsReady(true);
    })();

    return () => {
      cancelled = true;
      wsRef.current?.destroy();
      wsRef.current = null;
      regionsRef.current = null;
      if (blobUrlRef.current) {
        URL.revokeObjectURL(blobUrlRef.current);
        blobUrlRef.current = null;
      }
      setIsReady(false);
      setIsPlaying(false);
      setCurrentTime(0);
    };
  }, [roundId]);

  useEffect(() => {
    wsRef.current?.zoom(pixelsPerSecond);
  }, [pixelsPerSecond]);

  const handleSeek = useCallback((seekMs: number) => {
    if (wsRef.current) {
      const dur = wsRef.current.getDuration();
      wsRef.current.seekTo(Math.max(0, seekMs / 1000 / dur));
      wsRef.current.play();
    }
  }, []);

  const currentFrameSeq = useMemo(() => {
    if (frameMarkers.length === 0) return null;

    const targetMs = currentTime * 1000;
    let lo = 0;
    let hi = frameMarkers.length - 1;
    let idx = -1;

    while (lo <= hi) {
      const mid = (lo + hi) >>> 1;
      if (frameMarkers[mid].startMs <= targetMs) {
        idx = mid;
        lo = mid + 1;
      } else {
        hi = mid - 1;
      }
    }

    const result = idx >= 0 ? frameMarkers[idx].seq : null;
    if (currentTime > 0 || result != null) {
      console.log(`[Timeline:${roundId.slice(-6)}] currentFrameSeq: time=${currentTime.toFixed(4)}s, markers=${frameMarkers.length}, idx=${idx}, seq=${result}`);
    }
    return result;
  }, [frameMarkers, currentTime]);

  useEffect(() => {
    if (currentFrameSeq == null || !showFrames) return;
    document.querySelector(`[data-frame-seq="${currentFrameSeq}"]`)?.scrollIntoView({ block: 'nearest' });
  }, [currentFrameSeq, showFrames]);

  useEffect(() => {
    console.log(`[Timeline:${roundId.slice(-6)}] showFrames effect: isReady=${isReady}, frameList=${frameList.length}`);
    if (isReady && frameList.length > 0) {
      setShowFrames(true);
    }
  }, [isReady, frameList.length]);

  const handlePlay = () => wsRef.current?.play();
  const handlePause = () => wsRef.current?.pause();
  const handleStop = () => wsRef.current?.stop();

  return (
    <Stack gap={4}>
      <style>{`@keyframes frameCellSweep{0%{background-position:0% 0}100%{background-position:100% 0}}`}</style>
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

      {sorted.length > 0 && (
        <Stack gap={4} mt={8}>
          {sorted.map((d, i) => {
            const label = stepLabels[d.data_type] ?? d.data_type;
            const meta = d.metadata;
            const procMs = meta?.duration_ms as number | undefined;
            const audioDurMs = meta?.audio_duration_ms as number | undefined;

            let headerText: string;
            if (d.data_type === 'input_audio') {
              headerText = audioDurMs != null ? `✓ ${(audioDurMs / 1000).toFixed(1)}s` : '';
            } else if (d.data_type === 'tts') {
              const dataPart = audioDurMs != null ? `✓ ${(audioDurMs / 1000).toFixed(1)}s` : '✓';
              const proc = procMs != null ? `⏱${(procMs / 1000).toFixed(procMs < 1000 ? 1 : 0)}s` : '';
              headerText = `${dataPart}${proc ? `｜${proc}` : ''}`;
            } else {
              const txt = d.text
                ? d.text.length <= 10
                  ? `"${d.text}"`
                  : `"${d.text.slice(0, 10)}..."(${d.text.length}字)`
                : '✓';
              const proc = procMs != null ? `⏱${(procMs / 1000).toFixed(procMs < 1000 ? 1 : 0)}s` : '';
              headerText = `${txt}${proc ? `｜${proc}` : ''}`;
            }

            return (
              <Box key={d.id}>
                <Group gap={4} wrap="nowrap">
                  <Text size="xs" fw={600} c={stepColors[d.data_type] ?? 'gray'}>
                    {i + 1}. {label}
                  </Text>
                  <Text size="xs" c="dimmed">
                    {headerText}
                  </Text>
                </Group>
                {d.text && d.data_type !== 'input_audio' && (
                  <Text pl={16} size="xs" c="dimmed" style={{ lineHeight: 1.5 }}>
                    &ldquo;{d.text}&rdquo;
                  </Text>
                )}
              </Box>
            );
          })}
        </Stack>
      )}

      {frameList.length > 0 && (
        <>
          <Box mt={8} style={{ borderTop: '1px solid var(--mantine-color-gray-3)' }} />
          <Group
            gap={4}
            style={{ cursor: 'pointer' }}
            onClick={() => setShowFrames(!showFrames)}
          >
            {showFrames ? <IconChevronDown size={14} /> : <IconChevronRight size={14} />}
            <Text size="xs" fw={600}>
              帧记录 ({frameList.length}条)
            </Text>
            <Box ml="auto" onClick={(e) => e.stopPropagation()}>
              <Switch
                size="xs"
                label="波形同步"
                checked={syncMode}
                onChange={(e) => setSyncMode(e.currentTarget.checked)}
              />
            </Box>
          </Group>
          {showFrames && (
            <Box
              style={{
                display: 'flex',
                flexWrap: 'wrap',
                gap: 4,
                maxHeight: 300,
                overflowY: 'auto',
                marginTop: 8,
                padding: 4,
              }}
            >
              {frameList.map((f) => (
                <FrameGridCell
                  key={f.seq}
                  seq={f.seq}
                  dir={f.dir}
                  seekMs={f.seekMs}
                  color={f.color}
                  isActive={f.seq === currentFrameSeq}
                  detail={f.detail}
                  kind={f.kind}
                  onSeek={handleSeek}
                />
              ))}
            </Box>
          )}
        </>
      )}
    </Stack>
  );
}
