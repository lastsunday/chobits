import { getSessionRounds, listRoundData, listSessions } from '@/api';
import { LatencyWaterfall } from '@/components/LatencyWaterfall';
import { SessionMinimap } from '@/components/SessionMinimap';
import { Timeline } from '@/components/Timeline';
import type { RoundData } from '@/data/round-data';
import type { SessionListItem, TurnStep } from '@/data/session';
import {
  Badge,
  Box,
  Button,
  CopyButton,
  Grid,
  Group,
  Pagination,
  Paper,
  Select,
  Stack,
  Table,
  Text,
  TextInput,
  Title,
} from '@mantine/core';
import { DateTimePicker } from '@mantine/dates';
import { useQueries, useQuery } from '@tanstack/react-query';
import { createFileRoute } from '@tanstack/react-router';
import dayjs from 'dayjs';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { IconChevronDown, IconChevronRight } from '@tabler/icons-react';

export const Route = createFileRoute('/_pathlessLayout/admin/sessions/')({
  component: RouteComponent,
});

const stepColors: Record<string, string> = {
  input_audio: 'green',
  asr: 'cyan',
  text: 'yellow',
  llm: 'violet',
  tts: 'red',
};

const stepLabels: Record<string, string> = {
  input_audio: '语音输入',
  asr: '语音识别',
  text: '文本输入',
  llm: '大模型',
  tts: '语音合成',
};

const barHexColors: Record<string, string> = {
  input_audio: '#40c057',
  asr: '#15aabf',
  text: '#fab005',
  llm: '#7950f2',
  tts: '#fa5252',
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

  const processSteps = steps.filter((s) =>
    s.step !== 'input_audio' && s.step !== 'text'
    && s.has_data && s.duration_ms != null && s.duration_ms >= 0,
  );
  const totalProcessMs = processSteps.reduce((sum, s) => sum + s.duration_ms!, 0);

  const runSteps = steps.filter((s) =>
    s.step !== 'text' && s.has_data,
  ).filter((s) => {
    const v = s.step === 'input_audio' || s.step === 'tts'
      ? s.audio_duration_ms : s.duration_ms;
    return v != null && v >= 0;
  });
  const totalRunMs = runSteps.reduce((sum, s) => {
    return sum + (s.step === 'input_audio' || s.step === 'tts'
      ? (s.audio_duration_ms ?? 0) : (s.duration_ms ?? 0));
  }, 0);

  return (
    <>
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
      {totalProcessMs > 0 && (
        <>
          <Box mt={4} mb={6} ml="lg" style={{ borderTop: '1px dashed var(--mantine-color-gray-3)' }} />
          <Box ml="lg">
            <Group gap={8} mb={2} wrap="nowrap">
              <Text style={{ width: 56, flexShrink: 0 }} c="dimmed" size="xs">处理耗时</Text>
              <Box style={{ display: 'flex', flex: 1, height: 14, borderRadius: 3, overflow: 'hidden', background: 'var(--mantine-color-gray-1)' }}>
                {processSteps.map((s) => {
                  const pct = totalProcessMs > 0 ? ((s.duration_ms ?? 0) / totalProcessMs) * 100 : 0;
                  return (
                    <Box
                      key={s.step}
                      style={{ width: `${Math.max(pct, 0)}%`, height: '100%', background: barHexColors[s.step] ?? '#868e96', display: 'flex', alignItems: 'center', justifyContent: 'center' }}
                    >
                      {pct >= 15 && (
                        <Text size="xs" c="white" fw={600} style={{ lineHeight: '14px', textShadow: '0 1px 2px rgba(0,0,0,0.3)' }}>
                          {Math.round(pct)}%
                        </Text>
                      )}
                    </Box>
                  );
                })}
              </Box>
              <Text size="xs" fw={600} style={{ whiteSpace: 'nowrap' }}>
                {(totalProcessMs / 1000).toFixed(totalProcessMs < 1000 ? 1 : 0)}s
              </Text>
            </Group>
            <Group gap={8} wrap="nowrap">
              <Text style={{ width: 56, flexShrink: 0 }} c="dimmed" size="xs">运行时间</Text>
              <Box style={{ display: 'flex', flex: 1, height: 14, borderRadius: 3, overflow: 'hidden', background: 'var(--mantine-color-gray-1)' }}>
                {runSteps.map((s) => {
                  const v = s.step === 'input_audio' || s.step === 'tts'
                    ? (s.audio_duration_ms ?? 0) : (s.duration_ms ?? 0);
                  const pct = totalRunMs > 0 ? (v / totalRunMs) * 100 : 0;
                  return (
                    <Box
                      key={s.step}
                      style={{ width: `${Math.max(pct, 0)}%`, height: '100%', background: barHexColors[s.step] ?? '#868e96', display: 'flex', alignItems: 'center', justifyContent: 'center' }}
                    >
                      {pct >= 15 && (
                        <Text size="xs" c="white" fw={600} style={{ lineHeight: '14px', textShadow: '0 1px 2px rgba(0,0,0,0.3)' }}>
                          {Math.round(pct)}%
                        </Text>
                      )}
                    </Box>
                  );
                })}
              </Box>
              <Text size="xs" fw={600} style={{ whiteSpace: 'nowrap' }}>
                {(totalRunMs / 1000).toFixed(totalRunMs < 1000 ? 1 : 0)}s
              </Text>
            </Group>
          </Box>
        </>
      )}
    </>
  );
}

function getSessionTitle(session: SessionListItem): string {
  const firstTurn = session.turns[0];
  if (firstTurn) {
    for (const step of firstTurn.steps) {
      if (step.has_data && step.text) {
        return step.text.length > 40
          ? step.text.slice(0, 40) + '...'
          : step.text;
      }
    }
  }
  return '#' + session.session_id.slice(-8);
}

function formatDuration(ms: number): string {
  if (ms <= 0) return '';
  if (ms < 1000) return `${ms}ms`;
  const sec = ms / 1000;
  if (sec < 60) return `${sec.toFixed(sec < 10 ? 1 : 0)}s`;
  const min = Math.floor(sec / 60);
  const remainSec = Math.round(sec % 60);
  return `${min}m${remainSec}s`;
}

function RouteComponent() {
  const { t } = useTranslation();

  const [page, setPage] = useState(1);
  const [searchInput, setSearchInput] = useState('');
  const [search, setSearch] = useState('');
  const [dateFromInput, setDateFromInput] = useState<string | null>(null);
  const [dateToInput, setDateToInput] = useState<string | null>(null);
  const [dateFrom, setDateFrom] = useState<string | null>(null);
  const [dateTo, setDateTo] = useState<string | null>(null);
  const [sortOrder, setSortOrder] = useState<string>('desc');

  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);

  const [expandedRoundMap, setExpandedRoundMap] = useState<Record<string, boolean>>({});
  const roundElementsRef = useRef<Record<string, HTMLElement | null>>({});
  const prevRoundIdsRef = useRef<string>('');

  const { data, isLoading } = useQuery({
    queryKey: ['sessions', page, search, dateFrom, dateTo, sortOrder],
    queryFn: () =>
      listSessions({
        page,
        page_size: 20,
        ...(search ? { search } : {}),
        ...(dateFrom ? { date_from: dayjs(dateFrom).toISOString() } : {}),
        ...(dateTo ? { date_to: dayjs(dateTo).toISOString() } : {}),
        sort_order: sortOrder as 'asc' | 'desc',
      }),
  });

  const { data: rounds = [] } = useQuery({
    queryKey: ['session-rounds', selectedSessionId],
    queryFn: () => getSessionRounds(selectedSessionId!),
    enabled: !!selectedSessionId,
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

  const handleSearch = () => {
    setPage(1);
    setSearch(searchInput);
    setDateFrom(dateFromInput);
    setDateTo(dateToInput);
  };

  const handleSelectSession = (sessionId: string) => {
    setSelectedSessionId((prev) => (prev === sessionId ? null : sessionId));
  };

  const sortOptions = [
    { value: 'desc', label: t('sessions.sort_created_desc') },
    { value: 'asc', label: t('sessions.sort_created_asc') },
  ];

  const selectedRoundsTotalMs = rounds.reduce(
    (sum, r) => sum + r.steps.reduce((s, st) => s + (st.duration_ms ?? 0), 0),
    0,
  );

  const allDataReady = rounds.length > 0
    && roundDataQueries.every((q) => q.data !== undefined);

  return (
    <>
      <Title mb="lg">{t('sessions.title')}</Title>

      <Grid>
        <Grid.Col span={{ base: 12, md: 5 }}>
          <Stack>
            <Paper withBorder shadow="sm" p="md" radius="md">
              <Stack>
                <Group gap="sm">
                  <TextInput
                    placeholder={t('sessions.search')}
                    value={searchInput}
                    onChange={(e) => setSearchInput(e.currentTarget.value)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') handleSearch();
                    }}
                    style={{ flex: 1 }}
                  />
                  <Button onClick={handleSearch}>{t('sessions.search_btn')}</Button>
                </Group>

                <Group grow>
                  <DateTimePicker
                    placeholder={t('sessions.date_from')}
                    value={dateFromInput}
                    onChange={setDateFromInput}
                    clearable
                    valueFormat="YYYY-MM-DD HH:mm"
                  />
                  <DateTimePicker
                    placeholder={t('sessions.date_to')}
                    value={dateToInput}
                    onChange={setDateToInput}
                    clearable
                    valueFormat="YYYY-MM-DD HH:mm"
                  />
                  <Select
                    data={sortOptions}
                    value={sortOrder}
                    onChange={(val) => {
                      if (!val) return;
                      setSortOrder(val);
                      setPage(1);
                    }}
                  />
                </Group>
              </Stack>
            </Paper>

            {isLoading && (
              <Text ta="center" py="xl">
                {t('loading')}
              </Text>
            )}

            {data && data.items.length > 0 && (
              <Paper withBorder shadow="sm" radius="md" style={{ overflow: 'hidden' }}>
                <Table>
                  <Table.Thead>
                    <Table.Tr>
                      <Table.Th style={{ width: 170, whiteSpace: 'nowrap' }}>时间</Table.Th>
                      <Table.Th>摘要</Table.Th>
                      <Table.Th style={{ width: 50, whiteSpace: 'nowrap' }}>轮次</Table.Th>
                      <Table.Th style={{ width: 70, whiteSpace: 'nowrap' }}>处理耗时</Table.Th>
                      <Table.Th style={{ width: 50, whiteSpace: 'nowrap' }}>操作</Table.Th>
                    </Table.Tr>
                  </Table.Thead>
                  <Table.Tbody>
                    {data.items.map((session) => {
                      const totalMs = session.turns.reduce(
                        (sum, turn) => sum + turn.steps.reduce((s, st) => s + (st.duration_ms ?? 0), 0),
                        0,
                      );
                      const isSelected = selectedSessionId === session.session_id;
                      return (
                        <Table.Tr
                          key={session.session_id}
                          onClick={() => handleSelectSession(session.session_id)}
                          style={{
                            cursor: 'pointer',
                            borderLeft: isSelected ? '3px solid var(--mantine-color-blue-5)' : '3px solid transparent',
                            background: isSelected ? 'var(--mantine-color-blue-0)' : undefined,
                          }}
                          onMouseEnter={(e) => {
                            if (!isSelected) e.currentTarget.style.background = 'var(--mantine-color-gray-0)';
                          }}
                          onMouseLeave={(e) => {
                            if (!isSelected) e.currentTarget.style.background = '';
                          }}
                        >
                          <Table.Td>
                            <Text
                              size="sm"
                              style={{ whiteSpace: 'nowrap' }}
                              title={session.create_datetime ? dayjs(session.create_datetime).format('YYYY-MM-DD HH:mm:ss') : undefined}
                            >
                              {session.create_datetime
                                ? dayjs(session.create_datetime).format('YYYY-MM-DD HH:mm')
                                : ''}
                            </Text>
                          </Table.Td>
                          <Table.Td>
                            <Text size="sm" truncate="end" style={{ maxWidth: 200 }} title={getSessionTitle(session)}>
                              {getSessionTitle(session)}
                            </Text>
                          </Table.Td>
                          <Table.Td>
                            <Text size="sm">{session.turn_count}</Text>
                          </Table.Td>
                          <Table.Td>
                            <Text size="sm">{formatDuration(totalMs)}</Text>
                          </Table.Td>
                          <Table.Td>
                            <Button
                              variant="subtle"
                              size="compact-xs"
                              onClick={(e) => { e.stopPropagation(); handleSelectSession(session.session_id); }}
                              px={4}
                            >
                              👁
                            </Button>
                          </Table.Td>
                        </Table.Tr>
                      );
                    })}
                  </Table.Tbody>
                </Table>
              </Paper>
            )}

            {data && data.items.length === 0 && (
              <Text ta="center" py="xl" c="dimmed">
                {t('sessions.select_hint')}
              </Text>
            )}

            {data && data.total > data.page_size && (
              <Group justify="center">
                <Pagination
                  total={Math.ceil(data.total / data.page_size)}
                  value={page}
                  onChange={setPage}
                />
              </Group>
            )}
          </Stack>
        </Grid.Col>

        <Grid.Col span={{ base: 12, md: 7 }}>
          {selectedSessionId ? (
            <Stack>
              <Paper withBorder shadow="sm" p="md" radius="md">
                <Group justify="space-between" mb="md">
                  <Group gap="xs">
                    <CopyButton value={selectedSessionId}>
                      {({ copied, copy }) => (
                        <Group gap={2} wrap="nowrap">
                          <Text size="sm" fw={600} style={{ fontFamily: 'monospace' }}>
                            {selectedSessionId}
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
                    {selectedRoundsTotalMs > 0 && (
                      <Text size="xs" c="dimmed">
                        {(selectedRoundsTotalMs / 1000).toFixed(selectedRoundsTotalMs < 1000 ? 1 : 0)}s
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
            </Stack>
          ) : (
            <Paper withBorder shadow="sm" p="xl" radius="md" style={{ textAlign: 'center' }}>
              <Text c="dimmed">{t('sessions.select_hint')}</Text>
            </Paper>
          )}
        </Grid.Col>
      </Grid>
    </>
  );
}
