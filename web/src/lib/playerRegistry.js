/**
 * Opponent registry — local, remote, and future competition targets.
 */

import { PlayerType, TimeToMove, getEngineList } from './engineConfig.js';

/** Gorisanson view.js presets — Novice / Average / Good / Strong. */
export const GORISANSON_SIMULATIONS = {
  [TimeToMove.Intuition]: 2_500,
  [TimeToMove.Short]: 7_500,
  [TimeToMove.Medium]: 20_000,
  [TimeToMove.Long]: 60_000,
};

export const TIME_PRESETS = [
  {
    id: TimeToMove.Intuition,
    label: 'Intuition',
    shortLabel: 'Intuition',
  },
  {
    id: TimeToMove.Short,
    label: 'Short',
    shortLabel: 'Short',
  },
  {
    id: TimeToMove.Medium,
    label: 'Medium',
    shortLabel: 'Medium',
  },
  {
    id: TimeToMove.Long,
    label: 'Long',
    shortLabel: 'Long',
  },
];

const GORISANSON_ENGINE = {
  kind: 'local',
  name: 'Gorisanson MCTS',
  key: PlayerType.GorisansonMCTS,
  tooltip: 'Local MCTS — first boss (github.com/gorisanson/quoridor-ai)',
  uctConst: 0.2,
  simulations: GORISANSON_SIMULATIONS,
};

const PLACEHOLDER_ENGINES = [
  {
    kind: 'placeholder',
    name: 'Titanium (Rust)',
    key: PlayerType.Titanium,
    tooltip: 'Our engine — αβ search coming in episode 07',
    disabled: true,
  },
  {
    kind: 'placeholder',
    name: 'pavlosdais (C αβ)',
    key: PlayerType.Pavlosdais,
    tooltip: 'Competition baseline — not wired yet',
    disabled: true,
  },
];

export function getAllEngineConfigs() {
  const remote = getEngineList().map((entry) => ({
    ...entry,
    kind: 'remote',
  }));
  return [GORISANSON_ENGINE, ...remote, ...PLACEHOLDER_ENGINES];
}

export function getPlayerOptionGroups() {
  return [
    {
      label: 'Human',
      options: [{ value: PlayerType.Human, label: 'Human', disabled: false }],
    },
    {
      label: 'Local — beat these first',
      options: [
        {
          value: PlayerType.GorisansonMCTS,
          label: 'Gorisanson MCTS',
          disabled: false,
          tooltip: GORISANSON_ENGINE.tooltip,
        },
        {
          value: PlayerType.Titanium,
          label: 'Titanium (soon)',
          disabled: true,
        },
      ],
    },
    {
      label: 'Remote',
      options: [
        { value: PlayerType.IshtarV3, label: 'Ishtar', disabled: false },
        { value: PlayerType.KaAI, label: 'Ka', disabled: false },
      ],
    },
    {
      label: 'Competition (planned)',
      options: [
        { value: PlayerType.Pavlosdais, label: 'pavlosdais C', disabled: true },
      ],
    },
  ];
}

export function flattenPlayerOptions(groups) {
  return groups.flatMap((group) => group.options);
}

export function describeTimeBudget(players, timeMode, engineConfigs) {
  const aiTypes = players.filter((p) => p !== PlayerType.Human);
  if (aiTypes.length === 0) {
    return 'No AI selected — time preset applies when an engine is chosen.';
  }

  const lines = aiTypes.map((playerType) => {
    const config = engineConfigs.find((entry) => entry.key === playerType);
    if (!config) {
      return '';
    }
    return describeOneTimeBudget(config, timeMode);
  }).filter(Boolean);

  return lines.join(' · ');
}

function describeOneTimeBudget(config, timeMode) {
  if (!config) {
    return '';
  }

  if (config.kind === 'local' && config.simulations) {
    const sims = config.simulations[timeMode];
    return `${config.name}: ~${sims.toLocaleString()} MCTS rollouts`;
  }

  if (config.kind === 'remote' && config.visits) {
    const visits = config.visits[timeMode];
    const parallelism = config.settings?.parallelism?.[timeMode];
    let text = `${config.name}: ~${visits.toLocaleString()} visits`;
    if (parallelism) {
      text += ` (${parallelism} threads)`;
    }
    return text;
  }

  if (config.disabled) {
    return `${config.name}: coming soon`;
  }

  return '';
}

export function describeActiveSearchInfo(players, searchInfoByType, engineConfigs) {
  const aiTypes = players.filter((p) => p !== PlayerType.Human);
  const lines = aiTypes
    .map((playerType) =>
      describeSearchInfo(playerType, searchInfoByType[playerType], engineConfigs),
    )
    .filter(Boolean);
  return lines.join(' · ');
}

export function describeSearchInfo(playerType, searchInfo, engineConfigs) {
  if (!searchInfo || playerType === PlayerType.Human) {
    return '';
  }
  const config = engineConfigs.find((entry) => entry.key === playerType);
  if (config?.kind === 'local' && searchInfo.time != null) {
    const sims = searchInfo.simulations?.toLocaleString() ?? '?';
    return `Last think: ${(searchInfo.time / 1000).toFixed(1)}s · ${sims} sims`;
  }
  if (config?.kind === 'remote') {
    const parts = [];
    if (searchInfo.time != null) {
      parts.push(`${searchInfo.time}ms`);
    }
    if (searchInfo.visits != null) {
      parts.push(`${searchInfo.visits.toLocaleString()} visits`);
    }
    return parts.length ? `Last think: ${parts.join(' · ')}` : '';
  }
  return '';
}
