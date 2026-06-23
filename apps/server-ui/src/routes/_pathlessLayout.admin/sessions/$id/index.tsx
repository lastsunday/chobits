import { getSessionRounds, listRoundData } from '@/api';
import { LatencyWaterfall } from '@/components/LatencyWaterfall';
import { SessionMinimap } from '@/components/SessionMinimap';
import { Timeline } from '@/components/Timeline';
import type { RoundData } from '@/data/round-data';
import type { TurnStep } from '@/data/session';
import {
  Anchor,
  Badge,
  Box,
  Button,
  CopyButton,
  Group,
  Paper,
  Text,
  Title,
} from '@mantine/core';
import { useQueries, useQuery } from '@tanstack/react-query';
import { createFileRoute, useParams, useRouter } from '@tanstack/react-router';
import dayjs from 'dayjs';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { IconChevronDown, IconChevronRight } from '@tabler/icons-react';

export const Route = createFileRoute(
  '/_pathlessLayout/admin/sessions/$id/',
)({
  component: RouteComponent,
});

const stepLabels: Record<string, string> = {
  input_audio: '语音输入',
  asr: '语音识别',
  text: '文本输入',
  llm: '大模型',
  tts: '语音合成',
};

const stepColors: Record<string, string> = {
  input_audio: 'green',
  asr: 'cyan',
  text: 'yellow',
  llm: 'violet',
  tts: 'red',
};

function stepValueMs(s: TurnStep): number | null {
  if (s.step === 'input_audio' || s.step === 'tts') {
    return s.audio_duration_ms;
  }
  return s.duration_ms;
}

function RoundSummary({ steps }: { steps: TurnStep[] }) {
  const valid = steps.filter((s) => {
    const v = stepValueMs(s);
    return v != null && s.has_data;
  });
  if (valid.length === 0) return null;

  return (
    <Group gap={4} pl="lg" wrap="wrap">
      {valid.map((s, i) => {
        const v = stepValueMs(s)!;
        const proc = s.duration_ms != null
          ? `⏱${(s.duration_ms / 1000).toFixed(s.duration_ms < 1000 ? 1 : 0)}s`
          : '';
        const label = stepLabels[s.step] ?? s.step;
        const dur = `${(v / 1000).toFixed(v < 1000 ? 1 : 0)}s`;

        let badgeText: string;
        if (s.step === 'input_audio') {
          badgeText = `${label} ${dur}`;
        } else if (s.step === 'tts') {
          badgeText = `${label} ${dur}${proc ? ' ' + proc : ''}`;
        } else {
          const txt = s.text
            ? ` "${s.text.slice(0, 20)}${s.text.length > 20 ? '...' : ''}"`
            : '';
          badgeText = `${label}${txt}${proc ? ' ' + proc : ''}`;
        }

        return (
          <Group key={s.step} gap={2} wrap="nowrap" style={{ flexShrink: 0 }}>
            {i > 0 && <Text c="dimmed" size="xs">→</Text>}
            <Badge
              color={stepColors[s.step] ?? 'gray'}
              variant="light"
              size="sm"
              style={{ textTransform: 'none', fontWeight: 400, fontSize: 10 }}
            >
              {badgeText}
            </Badge>
          </Group>
        );
      })}
    </Group>
  );
}

function RouteComponent() {
  const { t } = useTranslation();
  const router = useRouter();
  const { id } = useParams({ from: Route.id });

  const [expandedRoundMap, setExpandedRoundMap] = useState<Record<string, boolean>>({});
  const roundElementsRef = useRef<Record<string, HTMLElement | null>>({});
  const prevRoundIdsRef = useRef<string>('');

  const { data: rounds = [], isLoading } = useQuery({
    queryKey: ['session-rounds', id],
    queryFn: () => getSessionRounds(id),
  });

  const roundDataQueries = useQueries({
    queries: rounds.map((r) => ({
      queryKey: ['round-data', r.round_id],
      queryFn: () => listRoundData(r.round_id),
    })),
  });

  const roundDataMap = useMemo(() => {
    const map: Record<string, RoundData[]> = {};
    rounds.forEach((r, i) => {
      map[r.round_id] = roundDataQueries[i]?.data ?? [];
    });
    return map;
  }, [rounds, roundDataQueries]);

  useEffect(() => {
    const currentIds = rounds.map((r) => r.round_id).join(',');
    if (currentIds !== prevRoundIdsRef.current) {
      prevRoundIdsRef.current = currentIds;
      setExpandedRoundMap((prev) => {
        const next: Record<string, boolean> = {};
        rounds.forEach((r) => {
          next[r.round_id] = prev[r.round_id] ?? false;
        });
        return next;
      });
    }
  }, [rounds]);

  const toggleRound = (roundId: string) => {
    setExpandedRoundMap((prev) => ({ ...prev, [roundId]: !prev[roundId] }));
  };

  const scrollToRound = (index: number) => {
    const round = rounds[index];
    if (!round) return;
    toggleRound(round.round_id);
    setTimeout(() => {
      roundElementsRef.current[round.round_id]?.scrollIntoView({
        behavior: 'smooth',
        block: 'start',
      });
    }, 0);
  };

  const totalMs = rounds.reduce(
    (sum, r) => sum + r.steps.reduce((s, st) => s + (st.duration_ms ?? 0), 0),
    0,
  );

  const allDataReady = rounds.length > 0
    && roundDataQueries.every((q) => q.data !== undefined);

  if (isLoading) {
    return <Text>{t('loading')}</Text>;
  }

  return (
    <>
      <Group mb="lg">
        <Anchor component="button" onClick={() => router.history.back()}>
          {t('sessions.detail.back')}
        </Anchor>
        <Title>{t('sessions.detail.title')}</Title>
      </Group>

      <Paper withBorder shadow="sm" p="md" radius="md">
        <Group justify="space-between" mb="md">
          <Group gap="xs">
            <CopyButton value={id}>
              {({ copied, copy }) => (
                <Group gap={2} wrap="nowrap">
                  <Text size="sm" fw={600} style={{ fontFamily: 'monospace' }}>
                    {id}
                  </Text>
                  <Button variant="subtle" size="compact-xs" onClick={copy} px={4}>
                    {copied ? '✓' : '复制'}
                  </Button>
                </Group>
              )}
            </CopyButton>
          </Group>
          <Group gap="md">
            <Text size="xs" c="dimmed">
              {rounds.length}轮
            </Text>
            {totalMs > 0 && (
              <Text size="xs" c="dimmed">
                {(totalMs / 1000).toFixed(totalMs < 1000 ? 1 : 0)}s
              </Text>
            )}
            <Text size="xs" c="dimmed">
              {rounds[0]?.create_datetime
                ? dayjs(rounds[0].create_datetime).format('YYYY-MM-DD HH:mm:ss')
                : ''}
            </Text>
          </Group>
        </Group>

        {rounds.length > 1 && (
          <Box mb="md">
            <SessionMinimap rounds={rounds} onRoundClick={scrollToRound} />
          </Box>
        )}

        {!allDataReady && (
          <Text size="sm" c="dimmed" ta="center" py="xl">
            {t('loading')}
          </Text>
        )}

        {allDataReady && rounds.map((round, idx) => {
          const isExpanded = expandedRoundMap[round.round_id] ?? false;
          return (
            <Box
              key={round.round_id}
              mb="lg"
              ref={(el) => { roundElementsRef.current[round.round_id] = el; }}
            >
              <Group
                gap="xs"
                mb={isExpanded ? 4 : 0}
                style={{ cursor: 'pointer' }}
                onClick={() => toggleRound(round.round_id)}
              >
                {isExpanded ? (
                  <IconChevronDown size={14} style={{ flexShrink: 0, color: 'var(--mantine-color-gray-5)' }} />
                ) : (
                  <IconChevronRight size={14} style={{ flexShrink: 0, color: 'var(--mantine-color-gray-5)' }} />
                )}
                <Text size="sm" fw={600}>
                  第{idx + 1}轮({t(`sessions.mode.${round.mode}`)})
                </Text>
                <Text size="xs" c="dimmed">
                  {round.create_datetime
                    ? dayjs(round.create_datetime).format('HH:mm:ss')
                    : ''}
                </Text>
              </Group>
              {isExpanded ? (
                <>
                  <LatencyWaterfall steps={round.steps} />
                  <Timeline
                    roundId={round.round_id}
                    dataItems={roundDataMap[round.round_id] ?? []}
                  />
                </>
              ) : (
                <RoundSummary steps={round.steps} />
              )}
            </Box>
          );
        })}
      </Paper>
    </>
  );
}
