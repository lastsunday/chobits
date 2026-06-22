import { getSessionRounds, listRoundData } from '@/api';
import { Timeline } from '@/components/Timeline';
import type { RoundData } from '@/data/round-data';
import {
  Anchor,
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
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';

export const Route = createFileRoute(
  '/_pathlessLayout/admin/sessions/$id/',
)({
  component: RouteComponent,
});

function RouteComponent() {
  const { t } = useTranslation();
  const router = useRouter();
  const { id } = useParams({ from: Route.id });

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
    </>
  );
}
