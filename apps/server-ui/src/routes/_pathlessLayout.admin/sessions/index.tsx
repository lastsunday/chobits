import { getSessionRounds, listRoundData, listSessions } from '@/api';
import { StepBadge } from '@/components/RoundStepBadge';
import { Timeline } from '@/components/Timeline';
import type { RoundData } from '@/data/round-data';
import type { SessionListItem } from '@/data/session';
import {
  Box,
  Button,
  CopyButton,
  Grid,
  Group,
  Pagination,
  Paper,
  Select,
  Stack,
  Text,
  TextInput,
  Title,
} from '@mantine/core';
import { DateTimePicker } from '@mantine/dates';
import { useQueries, useQuery } from '@tanstack/react-query';
import { createFileRoute } from '@tanstack/react-router';
import dayjs from 'dayjs';
import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

export const Route = createFileRoute('/_pathlessLayout/admin/sessions/')({
  component: RouteComponent,
});

function SessionCard({
  session,
  isSelected,
  onClick,
  t,
}: {
  session: SessionListItem;
  isSelected: boolean;
  onClick: () => void;
  t: (key: string, opts?: Record<string, unknown>) => string;
}) {
  const totalMs = session.turns.reduce(
    (sum, turn) => sum + turn.steps.reduce((s, st) => s + (st.duration_ms ?? 0), 0),
    0,
  );

  return (
    <Box
      style={{
        borderBottom: '1px solid var(--mantine-color-gray-3)',
        padding: 'var(--mantine-spacing-xs) var(--mantine-spacing-md)',
        cursor: 'pointer',
        background: isSelected ? 'var(--mantine-color-blue-0)' : undefined,
      }}
      onClick={onClick}
      onMouseEnter={(e) => {
        if (!isSelected) e.currentTarget.style.background = 'var(--mantine-color-gray-0)';
      }}
      onMouseLeave={(e) => {
        if (!isSelected) e.currentTarget.style.background = '';
      }}
    >
      <Group justify="space-between" mb={4}>
        <Group gap="xs">
          <Text size="sm" fw={600} style={{ whiteSpace: 'nowrap' }}>
            {session.create_datetime
              ? dayjs(session.create_datetime).format('YYYY-MM-DD HH:mm:ss')
              : ''}
          </Text>
          <CopyButton value={session.session_id}>
            {({ copied, copy }) => (
              <Group gap={2} wrap="nowrap">
                <Text size="xs" c="dimmed" style={{ fontFamily: 'monospace' }}>
                  {session.session_id}
                </Text>
                <Button variant="subtle" size="compact-xs" onClick={(e) => { e.stopPropagation(); copy(); }} px={4}>
                  {copied ? '✓' : '复制'}
                </Button>
              </Group>
            )}
          </CopyButton>
        </Group>
        <Group gap="md">
          <Text size="xs" c="dimmed">
            {session.turn_count}轮
          </Text>
          {totalMs > 0 && (
            <Text size="xs" c="dimmed">
              {(totalMs / 1000).toFixed(totalMs < 1000 ? 1 : 0)}s
            </Text>
          )}
        </Group>
      </Group>

      {session.turns[0] && (() => {
        const turn = session.turns[0];
        const turnEmpty = turn.steps.every((s) => !s.has_data);
        return (
          <Box pl="lg" style={turnEmpty ? { opacity: 0.4 } : undefined}>
            <Group gap={4} align="center" wrap="wrap">
              <Text size="xs" fw={600} style={{ whiteSpace: 'nowrap' }}>
                第{turn.turn_index}轮({t(`sessions.mode.${turn.mode}`)})
              </Text>
              <Text size="xs" c="dimmed">
                {turn.create_datetime
                  ? dayjs(turn.create_datetime).format('HH:mm:ss')
                  : ''}
              </Text>
              {turn.steps.map((s, i) => (
                <Group key={s.step} gap={2} wrap="nowrap">
                  {i > 0 && <Text c="dimmed" size="xs">→</Text>}
                  <StepBadge step={s} />
                </Group>
              ))}
            </Group>
          </Box>
        );
      })()}
    </Box>
  );
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

            {data && (
              <Stack gap={0}>
                {data.items.map((session) => (
                  <SessionCard
                    key={session.session_id}
                    session={session}
                    isSelected={selectedSessionId === session.session_id}
                    onClick={() => handleSelectSession(session.session_id)}
                    t={t}
                  />
                ))}
              </Stack>
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

                {!allDataReady && (
                  <Text size="sm" c="dimmed" ta="center" py="xl">
                    {t('loading')}
                  </Text>
                )}

                {allDataReady && rounds.map((round, idx) => (
                  <Box key={round.round_id} mb="lg">
                    <Group gap="xs" mb={4}>
                      <Text size="sm" fw={600}>
                        第{idx + 1}轮({t(`sessions.mode.${round.mode}`)})
                      </Text>
                      <Text size="xs" c="dimmed">
                        {round.create_datetime
                          ? dayjs(round.create_datetime).format('HH:mm:ss')
                          : ''}
                      </Text>
                    </Group>
                    <Timeline
                      roundId={round.round_id}
                      dataItems={roundDataMap[round.round_id] ?? []}
                    />
                  </Box>
                ))}
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
