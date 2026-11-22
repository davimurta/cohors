import { EmptyState } from '../empty/EmptyState.tsx';
import { FinishingView } from '../history/FinishingView.tsx';
import { HistoryView } from '../history/HistoryView.tsx';
import { TranscriptView } from '../history/TranscriptView.tsx';
import { SessionView } from '../session/SessionView.tsx';
import { SplitView } from '../split/SplitView.tsx';
import { inTab } from '../state/sessions.ts';
import { useAppStore } from '../state/store.ts';

/** The area right of the sidebar for the selected session. */
export function MainView() {
  const selected = useAppStore((state) =>
    state.sessions.find((session) => session.id === state.selectedId),
  );
  const tab = useAppStore((state) => state.sidebarTab);
  const transcript = useAppStore((state) => state.transcript);
  const panes = useAppStore((state) => state.panes);
  const sessions = useAppStore((state) => state.sessions);

  if (selected === undefined || !inTab(selected, tab)) {
    return tab === 'active' ? <EmptyState /> : null;
  }
  if (transcript?.id === selected.id) {
    return <TranscriptView name={selected.name} transcript={transcript} />;
  }
  if (panes.length > 1) {
    const shown = panes
      .map((id) => sessions.find((session) => session.id === id))
      .filter((session) => session !== undefined);
    return <SplitView sessions={shown} />;
  }
  if (selected.status !== 'finished') {
    return <SessionView key={selected.id} session={selected} />;
  }
  if (selected.summaryStatus === 'pending') {
    return <FinishingView key={selected.id} session={selected} />;
  }
  return <HistoryView key={selected.id} session={selected} />;
}
