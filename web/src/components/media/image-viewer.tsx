import { useState } from "react";

type Props = { src: string; alt?: string };

export function ImageViewer({ src, alt }: Props) {
  const [expanded, setExpanded] = useState(false);
  return (
    <>
      <img
        src={src}
        alt={alt || "Generated image"}
        className="max-w-full max-h-80 rounded-lg cursor-pointer hover:opacity-90 transition-opacity"
        onClick={() => setExpanded(true)}
      />
      {expanded && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/80"
          onClick={() => setExpanded(false)}
        >
          <img src={src} alt={alt} className="max-w-[90vw] max-h-[90vh] rounded-lg" />
        </div>
      )}
    </>
  );
}
