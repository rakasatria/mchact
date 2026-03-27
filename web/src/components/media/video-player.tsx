type Props = { src: string; caption?: string };

export function VideoPlayer({ src, caption }: Props) {
  return (
    <div className="max-w-lg">
      <video controls className="w-full rounded-lg" preload="metadata">
        <source src={src} type="video/mp4" />
      </video>
      {caption && <p className="text-xs text-gray-400 mt-1">{caption}</p>}
    </div>
  );
}
