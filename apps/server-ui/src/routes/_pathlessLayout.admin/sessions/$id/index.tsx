import { getRound, listFrames, listRoundData } from '@/api';
import { Timeline } from '@/components/Timeline';
import { Anchor, Group, Paper, Text, Title } from '@mantine/core';
import { useQuery } from '@tanstack/react-query';
import { createFileRoute, useParams, useRouter } from '@tanstack/react-router';
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

  const { data: round, isLoading: roundLoading } = useQuery({
    queryKey: ['round', id],
    queryFn: () => getRound(id),
  });

  const { data: dataItems = [] } = useQuery({
    queryKey: ['round-data', id],
    queryFn: () => listRoundData(id),
  });

  const { data: framesData } = useQuery({
    queryKey: ['frames', id],
    queryFn: () => listFrames(id, { page: 1, page_size: 500 }),
  });

  if (roundLoading || !round) {
    return <Text>{t('loading')}</Text>;
  }

  return (
    <>
      <Group mb="lg">
        <Anchor
          component="button"
          onClick={() => router.history.back()}
        >
          {t('sessions.detail.back')}
        </Anchor>
        <Title>{t('sessions.detail.title')}</Title>
      </Group>

      <Paper withBorder shadow="sm" p="md" radius="md" mb="lg">
        <Text size="sm">
          <strong>{t('sessions.detail.round_id')}:</strong>{' '}
          <span style={{ fontFamily: 'monospace' }}>{round.id}</span>
        </Text>
        <Text size="sm">
          <strong>{t('sessions.detail.user_id')}:</strong>{' '}
          {round.user_id ?? '-'}
        </Text>
        <Text size="sm">
          <strong>{t('sessions.detail.created')}:</strong>{' '}
          {round.create_datetime ?? '-'}
        </Text>
        <Text size="sm">
          <strong>{t('sessions.detail.updated')}:</strong>{' '}
          {round.update_datetime ?? '-'}
        </Text>
      </Paper>

      {framesData && (
        <Paper withBorder shadow="sm" p="md" radius="md">
          <Timeline
            round={round}
            dataItems={dataItems}
            frames={framesData.items}
          />
        </Paper>
      )}
    </>
  );
}
