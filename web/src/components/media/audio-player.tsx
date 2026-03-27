type Props = { src: string; name?: string };

export function AudioPlayer({ src, name }: Props) {
  return (
    <div className="flex flex-col gap-1 p-2 rounded-lg bg-white/5 border border-white/10 max-w-sm">
      {name && <span className="text-xs text-gray-400">{name}</span>}
      <audio controls className="w-full h-8" preload="metadata">
        <source src={src} />
      </audio>
    </div>
  );
}
