import { getAudioBlob } from '@/api';
import type { Frame } from '@/data/frame';
import type { Round } from '@/data/round';
import type { RoundData } from '@/data/round-data';
import { Badge, Box, Paper, Stack, Text, Tooltip } from '@mantine/core';
import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

// --- types ----------------------------------------------------

interface BusinessItem {
  id: string;
  ts: number;
  type: string;
  text: string | null;
  dataId: string | null;
}

interface FrameGroup {
  key: string;
  startTs: number;
  endTs: number;
  frameType: string;
  dir: 'inbound' | 'outbound';
  count: number;
  seq: number;
}

type TimelineItem =
  | { kind: 'group'; group: FrameGroup }
  | { kind: 'single'; frame: Frame; frameType: string };

// --- helpers ---------------------------------------------------

function parseFrameType(detail: string): string {
  const m = detail.match(/^(?:Frame::|Ok\()?(\w+)/);
  return m?.[1] ?? '?';
}

function isType(detail: string, name: string): boolean {
  return detail.startsWith(`Frame::${name}`) || detail.startsWith(`Ok(${name}`);
}

function getTs(e: { create_datetime: string | null }): number {
  return new Date(e.create_datetime ?? 0).getTime();
}

// --- AudioButton ------------------------------------------------

function AudioButton({ roundId, dataId, label }: { roundId: string; dataId: string; label: string }) {
  const [url, setUrl] = useState<string | null>(null);

  useEffect(() => {
    let objUrl: string | null = null;
    getAudioBlob(roundId, dataId)
      .then((blob) => {
        objUrl = URL.createObjectURL(blob);
        setUrl(objUrl);
      })
      .catch(() => {});
    return () => {
      if (objUrl) URL.revokeObjectURL(objUrl);
    };
  }, [roundId, dataId]);

  if (!url) return <Text size="xs" c="dimmed">{label}</Text>;

  return (
    <Stack gap={2}>
      <Text size="xs">{label}</Text>
      <audio src={url} controls style={{ height: 24, maxWidth: 160 }} />
    </Stack>
  );
}

// --- color / label helpers ---------------------------------------

function businessColor(type: string): string {
  switch (type) {
    case 'input_audio': return 'grape';
    case 'asr': return 'blue';
    case 'llm': return 'teal';
    case 'tts': return 'pink';
    default: return 'gray';
  }
}

function businessLabel(type: string): string {
  switch (type) {
    case 'input_audio': return 'Voice';
    case 'asr': return 'ASR';
    case 'llm': return 'LLM';
    case 'tts': return 'TTS';
    default: return type;
  }
}

// --- sub-components ----------------------------------------------

function formatTime(ms: number): string {
  const s = Math.floor(ms / 1000);
  const min = Math.floor(s / 60);
  const sec = s % 60;
  return min > 0 ? `${min}m${sec}s` : `${sec}s`;
}

function XAxis({
  timeRange,
  pct,
}: {
  timeRange: { min: number; duration: number };
  pct: (ts: number) => number;
}) {
  const ticks = useMemo(() => {
    const count = 6;
    const step = timeRange.duration / count;
    const result: { ts: number; left: number }[] = [];
    for (let i = 0; i <= count; i++) {
      const ts = timeRange.min + step * i;
      result.push({ ts, left: pct(ts) });
    }
    return result;
  }, [timeRange, pct]);

  return (
    <Box style={{ position: 'relative', height: 20 }}>
      {ticks.map((tick, i) => (
        <Box
          key={i}
          style={{
            position: 'absolute',
            left: `${tick.left}%`,
            top: 0,
            transform: 'translateX(-50%)',
          }}
        >
          <Text size="xs" c="dimmed" style={{ fontSize: 10 }}>
            {formatTime(tick.ts)}
          </Text>
        </Box>
      ))}
    </Box>
  );
}

function BusinessBlock({
  item,
  roundId,
  left,
}: {
  item: BusinessItem;
  roundId: string;
  left: number;
}) {
  const { t } = useTranslation();
  const color = businessColor(item.type);
  const label = businessLabel(item.type);

  return (
    <Tooltip label={item.text ?? label}>
      <Paper
        withBorder
        shadow="sm"
        p="xs"
        radius="md"
        style={{
          position: 'absolute',
          left: `${left}%`,
          top: 18,
          transform: 'translateX(-50%)',
          minWidth: 60,
          maxWidth: 200,
          borderLeft: `3px solid var(--mantine-color-${color}-6)`,
        }}
      >
        <Stack gap={2}>
          <Badge size="sm" color={color} variant="light">
            {label}
          </Badge>
          {item.type === 'input_audio' && item.dataId && (
            <AudioButton roundId={roundId} dataId={item.dataId} label={t('sessions.detail.play')} />
          )}
          {item.type === 'tts' && (
            <>
              {item.text && (
                <Text size="xs" lineClamp={2} style={{ maxWidth: 160 }}>
                  {item.text}
                </Text>
              )}
              {item.dataId && (
                <AudioButton roundId={roundId} dataId={item.dataId} label={t('sessions.detail.play')} />
              )}
            </>
          )}
          {(item.type === 'asr' || item.type === 'llm') && item.text && (
            <Text size="xs" lineClamp={2} style={{ maxWidth: 160 }}>
              {item.text}
            </Text>
          )}
        </Stack>
      </Paper>
    </Tooltip>
  );
}

function FrameGroupBlock({
  group,
  left,
  width,
  roundId,
  findDataId,
}: {
  group: FrameGroup;
  left: number;
  width: number;
  roundId: string;
  findDataId: (ts: number, dataType: string) => string | null;
}) {
  const { t } = useTranslation();
  const color = group.dir === 'inbound' ? 'yellow' : 'cyan';
  const dataType = group.frameType === 'Voice' ? 'input_audio' : 'tts';
  const dataId = findDataId(group.startTs, dataType);

  return (
    <Tooltip label={`${group.frameType} \u00d7${group.count}`}>
      <Paper
        withBorder
        shadow="sm"
        p={4}
        radius="sm"
        style={{
          position: 'absolute',
          left: `${left}%`,
          width: `${Math.max(width, 2)}%`,
          top: group.dir === 'inbound' ? 14 : 50,
          minWidth: 50,
          maxWidth: 180,
          borderTop: `2px solid var(--mantine-color-${color}-6)`,
          opacity: 0.85,
        }}
      >
        <Stack gap={2} align="center">
          <Badge size="sm" color={color} variant="light">
            {group.frameType} \u00d7{group.count}
          </Badge>
          {dataId && (
            <AudioButton roundId={roundId} dataId={dataId} label={t('sessions.detail.play')} />
          )}
        </Stack>
      </Paper>
    </Tooltip>
  );
}

function FrameMarker({
  frame,
  frameType,
  left,
}: {
  frame: Frame;
  frameType: string;
  left: number;
}) {
  const color = frame.dir === 'inbound' ? 'yellow' : 'cyan';
  const topPos = frame.dir === 'inbound' ? '12px' : '52px';
  const arrow = frame.dir === 'inbound' ? '\u25B2' : '\u25BC';

  return (
    <Tooltip label={`#${frame.seq} ${frame.dir} ${frameType}`}>
      <Box
        style={{
          position: 'absolute',
          left: `${left}%`,
          top: topPos,
          transform: 'translateX(-50%)',
          cursor: 'pointer',
        }}
      >
        <Badge size="xs" color={color} variant="filled" style={{ fontSize: 9, padding: '0 2px' }}>
          {arrow}{frameType.slice(0, 4)}
        </Badge>
      </Box>
    </Tooltip>
  );
}

// --- main component -----------------------------------------------

interface TimelineProps {
  round: Round;
  dataItems: RoundData[];
  frames: Frame[];
}

export function Timeline({ round, dataItems, frames }: TimelineProps) {
  const { t } = useTranslation();

  // Build ordered business items
  const business: BusinessItem[] = useMemo(
    () =>
      dataItems
        .map((d) => ({
          id: d.id,
          ts: getTs(d),
          type: d.data_type,
          text: d.text,
          dataId: d.id,
        }))
        .sort((a, b) => a.ts - b.ts),
    [dataItems],
  );

  // Build frame groups (consecutive same-type / same-dir grouped)
  const groups: TimelineItem[] = useMemo(() => {
    const result: TimelineItem[] = [];
    const voice: Frame[] = [];
    const audio: Frame[] = [];

    const flushVoice = () => {
      if (voice.length === 0) return;
      result.push({
        kind: 'group',
        group: {
          key: `voice-${voice[0].id}`,
          startTs: getTs(voice[0]),
          endTs: getTs(voice[voice.length - 1]),
          frameType: 'Voice',
          dir: 'inbound',
          count: voice.length,
          seq: voice[0].seq,
        },
      });
      voice.length = 0;
    };

    const flushAudio = () => {
      if (audio.length === 0) return;
      result.push({
        kind: 'group',
        group: {
          key: `audio-${audio[0].id}`,
          startTs: getTs(audio[0]),
          endTs: getTs(audio[audio.length - 1]),
          frameType: 'AudioResult',
          dir: 'outbound',
          count: audio.length,
          seq: audio[0].seq,
        },
      });
      audio.length = 0;
    };

    for (const f of frames) {
      const detail = f.detail ?? '';
      if (isType(detail, 'Voice')) {
        flushAudio();
        voice.push(f);
      } else if (isType(detail, 'AudioResult')) {
        flushVoice();
        audio.push(f);
      } else if (isType(detail, 'Hello') || isType(detail, 'HelloResult')) {
        flushVoice(); flushAudio();
        result.push({ kind: 'single', frame: f, frameType: parseFrameType(detail) });
      } else if (isType(detail, 'Close') || isType(detail, 'CloseResult')) {
        flushVoice(); flushAudio();
        result.push({ kind: 'single', frame: f, frameType: 'Close' });
      } else if (isType(detail, 'Abort')) {
        flushVoice(); flushAudio();
        result.push({ kind: 'single', frame: f, frameType: 'Abort' });
      } else {
        flushVoice(); flushAudio();
        result.push({ kind: 'single', frame: f, frameType: parseFrameType(detail) });
      }
    }
    flushVoice();
    flushAudio();

    return result;
  }, [frames]);

  // Compute time range
  const timeRange = useMemo(() => {
    const allTs: number[] = [];
    for (const b of business) allTs.push(b.ts);
    for (const item of groups) {
      if (item.kind === 'group') {
        allTs.push(item.group.startTs);
        allTs.push(item.group.endTs);
      } else {
        allTs.push(getTs(item.frame));
      }
    }
    if (allTs.length === 0) return { min: 0, max: 1, duration: 1 };
    let min = allTs[0];
    let max = allTs[0];
    for (const t of allTs) {
      if (t < min) min = t;
      if (t > max) max = t;
    }
    if (min === max) return { min, max: max + 1, duration: 1 };
    return { min, max, duration: max - min };
  }, [business, groups]);

  const pct = (ts: number) => ((ts - timeRange.min) / timeRange.duration) * 100;
  const widthPct = (start: number, end: number) =>
    ((end - start) / timeRange.duration) * 100;

  const findDataId = (ts: number, dataType: string): string | null => {
    let best: BusinessItem | null = null;
    let bestDist = Infinity;
    for (const b of business) {
      if (b.type !== dataType) continue;
      const dist = Math.abs(b.ts - ts);
      if (dist < bestDist) {
        bestDist = dist;
        best = b;
      }
    }
    return best?.dataId ?? null;
  };

  return (
    <Box style={{ overflowX: 'auto', overflowY: 'visible' }}>
      <Box style={{ position: 'relative', minWidth: 600, width: '100%' }}>
        <XAxis timeRange={timeRange} pct={pct} />

        {/* Track 1: Business Events */}
        <Box
          style={{
            position: 'relative',
            height: 80,
            borderBottom: '1px solid var(--mantine-color-gray-3)',
            marginTop: 4,
          }}
        >
          <Text size="xs" c="dimmed" style={{ position: 'absolute', left: 0, top: -2, fontSize: 10 }}>
            {t('sessions.timeline.business')}
          </Text>
          {business.map((b) => (
            <BusinessBlock
              key={b.id}
              item={b}
              roundId={round.id}
              left={pct(b.ts)}
            />
          ))}
        </Box>

        {/* Track 2: Frames */}
        <Box
          style={{
            position: 'relative',
            height: 100,
            marginTop: 4,
          }}
        >
          <Text size="xs" c="dimmed" style={{ position: 'absolute', left: 0, top: -2, fontSize: 10 }}>
            {t('sessions.timeline.frames')}
          </Text>
          {groups.map((item) => {
            if (item.kind === 'group') {
              const g = item.group;
              return (
                <FrameGroupBlock
                  key={g.key}
                  group={g}
                  left={pct(g.startTs)}
                  width={widthPct(g.startTs, g.endTs)}
                  roundId={round.id}
                  findDataId={findDataId}
                />
              );
            }
            return (
              <FrameMarker
                key={item.frame.id}
                frame={item.frame}
                frameType={item.frameType}
                left={pct(getTs(item.frame))}
              />
            );
          })}
        </Box>
      </Box>
    </Box>
  );
}
