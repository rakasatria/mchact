import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faFile } from "@fortawesome/free-solid-svg-icons";

type Props = { url: string; name: string; size?: number };

export function FilePreview({ url, name, size }: Props) {
  const sizeText = size ? `${(size / 1024).toFixed(1)} KB` : "";
  return (
    <a
      href={url}
      download={name}
      className="flex items-center gap-2 p-2 rounded-lg bg-white/5 border border-white/10 hover:bg-white/10 transition-colors max-w-xs"
    >
      <FontAwesomeIcon icon={faFile} className="w-5 h-5 text-gray-400 shrink-0" />
      <div className="min-w-0">
        <div className="text-sm truncate">{name}</div>
        {sizeText && <div className="text-xs text-gray-500">{sizeText}</div>}
      </div>
    </a>
  );
}
