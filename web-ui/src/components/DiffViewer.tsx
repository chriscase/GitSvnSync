import ReactDiffViewer, { DiffMethod } from 'react-diff-viewer-continued';

interface Props {
  oldValue: string;
  newValue: string;
  leftTitle: string;
  rightTitle: string;
}

export default function DiffViewer({
  oldValue,
  newValue,
  leftTitle,
  rightTitle,
}: Props) {
  return (
    <ReactDiffViewer
      oldValue={oldValue}
      newValue={newValue}
      splitView={true}
      leftTitle={leftTitle}
      rightTitle={rightTitle}
      compareMethod={DiffMethod.LINES}
      useDarkTheme={false}
      styles={{
        contentText: {
          fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace',
          fontSize: '13px',
          lineHeight: '1.5',
        },
      }}
    />
  );
}
