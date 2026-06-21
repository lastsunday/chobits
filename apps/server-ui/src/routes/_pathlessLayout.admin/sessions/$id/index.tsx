import { getAudioBlob, getRound, listFrames, listRoundData } from '@/api';
import type { RoundData } from '@/data/round-data';
import {
  Anchor,
  Badge,
  Group,
  Pagination,
  Paper,
  Stack,
  Table,
  Text,
  Timeline,
  Title,
} from '@mantine/core';
import { useQuery } from '@tanstack/react-query';
import { createFileRoute, useParams, useRouter } from '@tanstack/react-router';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

export const Route = createFileRoute(
  '/_pathlessLayout/admin/sessions/$id/',
)({
  component: RouteComponent,
});

function AudioPlayer({ label, roundId, dataId }: { label: string; roundId: string; dataId: string }) {
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

  if (!url) {
    return <Text size="sm" c="dimmed">Loading audio...</Text>;
  }

  return (
    <Stack gap={4}>
      <Text size="sm">{label}</Text>
      <audio src={url} controls style={{ maxWidth: 320, height: 32 }} />
    </Stack>
  );
}

function getDataTypeColor(t: string): string {
  switch (t) {
    case 'input_audio': return 'grape';
    case 'asr': return 'blue';
    case 'llm': return 'teal';
    case 'tts': return 'pink';
    default: return 'gray';
  }
}

function DataSection({ item }: { item: RoundData }) {
  const { t } = useTranslation();
  const { id: roundId } = useParams({ from: Route.id });

  if (item.data_type === 'input_audio') {
    return <AudioPlayer label={t('sessions.detail.input_audio')} roundId={roundId} dataId={item.id} />;
  }

  if (item.data_type === 'asr') {
    return (
      <Stack gap={4}>
        <Text size="sm">{t('sessions.detail.asr')}</Text>
        <Text size="sm" c="dimmed" style={{ fontStyle: 'italic' }}>
          {item.text ?? '-'}
        </Text>
      </Stack>
    );
  }

  if (item.data_type === 'llm') {
    return (
      <Stack gap={4}>
        <Text size="sm">{t('sessions.detail.llm')}</Text>
        <Text size="sm">{item.text ?? '-'}</Text>
      </Stack>
    );
  }

  if (item.data_type === 'tts') {
    return (
      <Stack gap={4}>
        <Text size="sm">{t('sessions.detail.tts')}</Text>
        <Text size="sm" c="dimmed">{item.text ?? '-'}</Text>
        {item.data && (
          <AudioPlayer label="" roundId={roundId} dataId={item.id} />
        )}
      </Stack>
    );
  }

  return null;
}

function getFrameDirColor(dir: string): string {
  return dir === 'inbound' ? 'yellow' : 'cyan';
}

function RouteComponent() {
  const { t } = useTranslation();
  const router = useRouter();
  const { id } = useParams({ from: Route.id });
  const [framesPage, setFramesPage] = useState(1);

  const { data: round, isLoading: roundLoading } = useQuery({
    queryKey: ['round', id],
    queryFn: () => getRound(id),
  });

  const { data: dataItems } = useQuery({
    queryKey: ['round-data', id],
    queryFn: () => listRoundData(id),
  });

  const { data: framesData } = useQuery({
    queryKey: ['frames', id, framesPage],
    queryFn: () => listFrames(id, { page: framesPage, page_size: 50 }),
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

      {dataItems && dataItems.length > 0 && (
        <Paper withBorder shadow="sm" p="md" radius="md" mb="lg">
          <Title order={3} mb="md">
            Data
          </Title>
          <Timeline active={dataItems.length - 1} bulletSize={24} lineWidth={2}>
            {dataItems.map((item) => (
              <Timeline.Item
                key={item.id}
                bullet={<Badge size="xs" color={getDataTypeColor(item.data_type)} circle />}
              >
                <DataSection item={item} />
              </Timeline.Item>
            ))}
          </Timeline>
        </Paper>
      )}

      {framesData && (
        <Paper withBorder shadow="sm" p="md" radius="md">
          <Title order={3} mb="md">
            {t('sessions.detail.frames')}
          </Title>
          <Table striped highlightOnHover>
            <Table.Thead>
              <Table.Tr>
                <Table.Th>{t('sessions.frame_table.seq')}</Table.Th>
                <Table.Th>{t('sessions.frame_table.dir')}</Table.Th>
                <Table.Th>{t('sessions.frame_table.kind')}</Table.Th>
                <Table.Th>{t('sessions.frame_table.detail')}</Table.Th>
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {framesData.items.map((f) => (
                <Table.Tr key={f.id}>
                  <Table.Td>{f.seq}</Table.Td>
                  <Table.Td>
                    <Badge color={getFrameDirColor(f.dir)} size="sm">
                      {f.dir}
                    </Badge>
                  </Table.Td>
                  <Table.Td>{f.kind}</Table.Td>
                  <Table.Td>
                    <Text size="sm" lineClamp={1}>
                      {f.detail ?? '-'}
                    </Text>
                  </Table.Td>
                </Table.Tr>
              ))}
            </Table.Tbody>
          </Table>
          {framesData.total > framesData.page_size && (
            <Group justify="center" mt="md">
              <Pagination
                total={Math.ceil(framesData.total / framesData.page_size)}
                value={framesPage}
                onChange={setFramesPage}
              />
            </Group>
          )}
        </Paper>
      )}
    </>
  );
}
