import { listRounds } from '@/api';
import {
  Group,
  Pagination,
  Paper,
  Table,
  Text,
  TextInput,
  Title,
} from '@mantine/core';
import { useDebouncedValue } from '@mantine/hooks';
import { useQuery } from '@tanstack/react-query';
import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

export const Route = createFileRoute('/_pathlessLayout/admin/sessions/')({
  component: RouteComponent,
});

function RouteComponent() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [page, setPage] = useState(1);
  const [userId, setUserId] = useState('');
  const [debouncedUserId] = useDebouncedValue(userId, 300);

  const { data, isLoading } = useQuery({
    queryKey: ['rounds', page, debouncedUserId],
    queryFn: () =>
      listRounds({
        page,
        page_size: 20,
        ...(debouncedUserId ? { user_id: debouncedUserId } : {}),
      }),
  });

  return (
    <>
      <Title mb="lg">{t('sessions.title')}</Title>
      <Paper withBorder shadow="sm" p="md" radius="md">
        <TextInput
          mb="md"
          placeholder={t('sessions.filter_user_id')}
          value={userId}
          onChange={(e) => {
            setUserId(e.currentTarget.value);
            setPage(1);
          }}
        />
        <Table striped highlightOnHover>
          <Table.Thead>
            <Table.Tr>
              <Table.Th>{t('sessions.table.id')}</Table.Th>
              <Table.Th>{t('sessions.table.user_id')}</Table.Th>
              <Table.Th>{t('sessions.table.created')}</Table.Th>
              <Table.Th>{t('sessions.table.actions')}</Table.Th>
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody>
            {isLoading && (
              <Table.Tr>
                <Table.Td colSpan={4}>
                  <Text ta="center">{t('loading')}</Text>
                </Table.Td>
              </Table.Tr>
            )}
            {data?.items.map((round) => (
              <Table.Tr
                key={round.id}
                style={{ cursor: 'pointer' }}
                onClick={() =>
                  navigate({
                    to: '/admin/sessions/$id',
                    params: { id: round.id },
                  })
                }
              >
                <Table.Td>
                  <Text size="sm" style={{ fontFamily: 'monospace' }}>
                    {round.id.slice(0, 12)}...
                  </Text>
                </Table.Td>
                <Table.Td>{round.user_id ?? '-'}</Table.Td>
                <Table.Td>{round.create_datetime ?? '-'}</Table.Td>
                <Table.Td>
                  <Text size="sm" c="blue">
                    View
                  </Text>
                </Table.Td>
              </Table.Tr>
            ))}
          </Table.Tbody>
        </Table>
        {data && data.total > data.page_size && (
          <Group justify="center" mt="md">
            <Pagination
              total={Math.ceil(data.total / data.page_size)}
              value={page}
              onChange={setPage}
            />
          </Group>
        )}
      </Paper>
    </>
  );
}
