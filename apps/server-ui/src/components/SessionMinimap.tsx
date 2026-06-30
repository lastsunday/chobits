import { useTranslation } from 'react-i18next';
import type { SessionRound } from '@/data/session';
import { Box, Tooltip } from '@mantine/core';

const modeColors: Record<string, string> = {
  Auto: '#40c057',
  Manual: '#fab005',
  RealTime: '#15aabf',
  Text: '#7950f2',
};

interface SessionMinimapProps {
  rounds: SessionRound[];
  onRoundClick: (index: number) => void;
}

export function SessionMinimap({ rounds, onRoundClick }: SessionMinimapProps) {
  const { t } = useTranslation();
  if (rounds.length === 0) return null;

  return (
    <Box style={{ display: 'flex', gap: 4, flexWrap: 'wrap' }}>
      {rounds.map((r, i) => {
        const color = modeColors[r.mode] ?? '#868e96';
        return (
          <Tooltip key={r.round_id} label={`第${i + 1}轮(${t(`sessions.mode.${r.mode.toLocaleLowerCase()}`)})`}>
            <Box
              style={{
                width: 28,
                height: 28,
                borderRadius: 6,
                background: color,
                opacity: 0.75,
                cursor: 'pointer',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                transition: 'opacity 0.12s',
                fontWeight: 600,
                fontSize: 13,
                color: '#fff',
                textShadow: '0 1px 2px rgba(0,0,0,0.3)',
              }}
              onMouseEnter={(e) => { e.currentTarget.style.opacity = '1'; }}
              onMouseLeave={(e) => { e.currentTarget.style.opacity = '0.75'; }}
              onClick={(e) => {
                e.stopPropagation();
                onRoundClick(i);
              }}
            >
              {i + 1}
            </Box>
          </Tooltip>
        );
      })}
    </Box>
  );
}
