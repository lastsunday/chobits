import { useTranslation } from 'react-i18next';
import type { TurnStep } from '@/data/session';
import { Box, Group, Text, Tooltip } from '@mantine/core';

const stepColors: Record<string, string> = {
  asr: '#15aabf',
  text: '#fab005',
  llm: '#7950f2',
  tts: '#fa5252',
};

const stepLabels: Record<string, string> = {
  asr: '语音识别',
  text: '文本输入',
  llm: '大模型',
  tts: '语音合成',
};

function stepValue(s: TurnStep): number | null {
  return s.duration_ms;
}

interface LatencyWaterfallProps {
  steps: TurnStep[];
  title?: string;
}

export function LatencyWaterfall({ steps, title }: LatencyWaterfallProps) {
  const { t } = useTranslation();
  const heading = title ?? t('sessions.latency_waterfall');

  const validSteps = steps.filter((s) => {
    if (s.step === 'input_audio') return false;
    const v = stepValue(s);
    return v != null && v >= 0 && s.has_data;
  });
  if (validSteps.length === 0) return null;

  const maxDur = Math.max(...validSteps.map((s) => stepValue(s)!));
  const totalMs = validSteps.reduce((sum, s) => sum + stepValue(s)!, 0);

  return (
    <Box pl="lg" style={{ fontSize: 12 }}>
      <Text size="sm" fw={600} c="dimmed" mb={8}>
        {heading}
      </Text>

      {validSteps.map((step) => {
        const v = stepValue(step)!;
        const color = stepColors[step.step] ?? '#868e96';
        const label = stepLabels[step.step] ?? step.step;
        const pct = (v / maxDur) * 100;
        const isBottleneck = v === maxDur && validSteps.length > 1;
        return (
          <Group key={step.step} gap={8} mb={2} wrap="nowrap">
            <Text style={{ width: 56, flexShrink: 0 }} c="dimmed" size="xs">
              {label}
            </Text>
            <Tooltip
              label={`${(v / 1000).toFixed(v < 1000 ? 1 : 0)}s`}
            >
              <Box
                style={{
                  height: 14,
                  width: `${Math.max(pct, 4)}%`,
                  minWidth: 20,
                  maxWidth: '100%',
                  borderRadius: 3,
                  background: color,
                  opacity: isBottleneck ? 1 : 0.55,
                  transition: 'opacity 0.15s',
                }}
                onMouseEnter={(e) => { e.currentTarget.style.opacity = '1'; }}
                onMouseLeave={(e) => { e.currentTarget.style.opacity = isBottleneck ? '1' : '0.55'; }}
              />
            </Tooltip>
            <Text size="xs" fw={isBottleneck ? 700 : 400} c={isBottleneck ? 'red' : undefined}>
              {(v / 1000).toFixed(v < 1000 ? 1 : 0)}s
            </Text>
            {isBottleneck && (
              <Text size="xs" c="red" fw={600} style={{ whiteSpace: 'nowrap', flexShrink: 0 }}>
                {t('sessions.bottleneck')}
              </Text>
            )}
          </Group>
        );
      })}
      <Group
        gap={8}
        mt={4}
        pt={4}
        wrap="nowrap"
        style={{ borderTop: '1px dashed var(--mantine-color-gray-3)' }}
      >
        <Text style={{ width: 56, flexShrink: 0 }} c="dimmed" size="xs" fw={600}>
          总计
        </Text>
        <Box style={{ display: 'flex', width: '100%', height: 14, borderRadius: 3, overflow: 'hidden' }}>
          {validSteps.map((s) => {
            const v = stepValue(s)!;
            const pct = totalMs > 0 ? (v / totalMs) * 100 : 0;
            return (
              <Box
                key={s.step}
                style={{
                  width: `${pct}%`,
                  height: '100%',
                  background: stepColors[s.step] ?? '#868e96',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                }}
              >
                {pct >= 15 && (
                  <Text size="xs" c="white" fw={600} style={{ lineHeight: 1, textShadow: '0 1px 2px rgba(0,0,0,0.3)' }}>
                    {Math.round(pct)}%
                  </Text>
                )}
              </Box>
            );
          })}
        </Box>
        <Text size="xs" fw={600}>
          {(totalMs / 1000).toFixed(totalMs < 1000 ? 1 : 0)}s
        </Text>
      </Group>
    </Box>
  );
}
