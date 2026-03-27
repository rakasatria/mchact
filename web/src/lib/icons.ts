import type { IconDefinition } from "@fortawesome/fontawesome-svg-core";
import {
  faGear,
  faMicrochip,
  faPuzzlePiece,
  faPlug,
  faHashtag,
  faGlobe,
  faKey,
  faNetworkWired,
  faDove,
  faCubes,
  faComment,
  faEnvelope,
  faCircleNodes,
  faShieldHalved,
  faBell,
  faCommentDots,
  faPhotoFilm,
} from "@fortawesome/free-solid-svg-icons";
import {
  faTelegram,
  faDiscord,
  faSlack,
  faWeixin,
  faWhatsapp,
} from "@fortawesome/free-brands-svg-icons";

export const TAB_ICONS: Record<string, IconDefinition> = {
  general: faGear,
  model: faMicrochip,
  skills: faPuzzlePiece,
  mcp: faPlug,
  telegram: faTelegram,
  discord: faDiscord,
  irc: faHashtag,
  slack: faSlack,
  web: faGlobe,
  access: faKey,
  a2a: faNetworkWired,
  multimodal: faPhotoFilm,
};

export const CHANNEL_ICONS: Record<string, IconDefinition> = {
  slack: faSlack,
  feishu: faDove,
  weixin: faWeixin,
  matrix: faCubes,
  whatsapp: faWhatsapp,
  imessage: faComment,
  email: faEnvelope,
  nostr: faCircleNodes,
  signal: faShieldHalved,
  dingtalk: faBell,
  qq: faCommentDots,
};
