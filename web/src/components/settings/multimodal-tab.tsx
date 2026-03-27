import React from "react";
import { Text, TextField } from "@radix-ui/themes";
import { ConfigFieldCard } from "../config-field-card";
import { ConfigToggleCard } from "../config-toggle-card";
import { useSettings } from "../../context/settings-context";

export function MultimodalTab(): React.ReactElement {
  const {
    configDraft,
    setConfigField,
    sectionCardClass,
    sectionCardStyle,
    toggleCardClass,
    toggleCardStyle,
  } = useSettings();

  return (
    <>
      {/* TTS Section */}
      <div className={sectionCardClass} style={sectionCardStyle}>
        <Text size="3" weight="bold">
          Text-to-Speech (TTS)
        </Text>
        <Text size="1" color="gray" className="mt-1 block">
          Convert bot responses to audio. Supported providers:{" "}
          <code>edge</code>, <code>openai</code>, <code>elevenlabs</code>.
        </Text>
        <div className="mt-4 grid grid-cols-1 gap-3">
          <ConfigToggleCard
            label="tts_enabled"
            description={<>Enable text-to-speech for voice responses.</>}
            checked={Boolean(configDraft.tts_enabled)}
            onCheckedChange={(checked) =>
              setConfigField("tts_enabled", checked)
            }
            className={toggleCardClass}
            style={toggleCardStyle}
          />
        </div>
        <div className="mt-3 space-y-3">
          <ConfigFieldCard
            label="tts_provider"
            description={
              <>
                TTS backend. One of <code>edge</code>, <code>openai</code>,{" "}
                <code>elevenlabs</code>.
              </>
            }
          >
            <select
              className="mt-2 w-full rounded-md border border-[color:var(--mc-border-soft)] bg-transparent px-3 py-2 text-base text-[color:inherit] outline-none focus:border-[color:var(--mc-accent)]"
              value={String(configDraft.tts_provider || "edge")}
              onChange={(e) => setConfigField("tts_provider", e.target.value)}
            >
              <option value="edge">edge (free, no API key)</option>
              <option value="openai">openai</option>
              <option value="elevenlabs">elevenlabs</option>
            </select>
          </ConfigFieldCard>
          <ConfigFieldCard
            label="tts_voice"
            description={
              <>
                Voice identifier for the selected TTS provider (e.g.{" "}
                <code>en-US-JennyNeural</code> for edge,{" "}
                <code>alloy</code> for openai).
              </>
            }
          >
            <TextField.Root
              className="mt-2"
              value={String(configDraft.tts_voice || "")}
              onChange={(e) => setConfigField("tts_voice", e.target.value)}
              placeholder="en-US-JennyNeural"
            />
          </ConfigFieldCard>
          <ConfigFieldCard
            label="tts_api_key"
            description={
              <>
                API key for the TTS provider. Not required for{" "}
                <code>edge</code>. Leave blank to keep current secret
                unchanged.
              </>
            }
          >
            <TextField.Root
              className="mt-2"
              value={String(configDraft.tts_api_key || "")}
              onChange={(e) => setConfigField("tts_api_key", e.target.value)}
              placeholder="sk-..."
            />
          </ConfigFieldCard>
        </div>
      </div>

      {/* STT Section */}
      <div className={`${sectionCardClass} mt-4`} style={sectionCardStyle}>
        <Text size="3" weight="bold">
          Speech-to-Text (STT)
        </Text>
        <Text size="1" color="gray" className="mt-1 block">
          Transcribe voice messages. Supported providers:{" "}
          <code>openai</code>, <code>whisper-local</code>.
        </Text>
        <div className="mt-4 grid grid-cols-1 gap-3">
          <ConfigToggleCard
            label="stt_enabled"
            description={<>Enable speech-to-text transcription.</>}
            checked={Boolean(configDraft.stt_enabled)}
            onCheckedChange={(checked) =>
              setConfigField("stt_enabled", checked)
            }
            className={toggleCardClass}
            style={toggleCardStyle}
          />
        </div>
        <div className="mt-3 space-y-3">
          <ConfigFieldCard
            label="stt_provider"
            description={
              <>
                STT backend. <code>openai</code> uses the Whisper API;{" "}
                <code>whisper-local</code> runs on-device.
              </>
            }
          >
            <select
              className="mt-2 w-full rounded-md border border-[color:var(--mc-border-soft)] bg-transparent px-3 py-2 text-base text-[color:inherit] outline-none focus:border-[color:var(--mc-accent)]"
              value={String(configDraft.stt_provider || "openai")}
              onChange={(e) => setConfigField("stt_provider", e.target.value)}
            >
              <option value="openai">openai (Whisper API)</option>
              <option value="whisper-local">
                whisper-local (on-device)
              </option>
            </select>
          </ConfigFieldCard>
          <ConfigFieldCard
            label="stt_model"
            description={
              <>
                Model to use for transcription (e.g.{" "}
                <code>whisper-1</code> for openai, or a local model path for
                whisper-local).
              </>
            }
          >
            <select
              className="mt-2 w-full rounded-md border border-[color:var(--mc-border-soft)] bg-transparent px-3 py-2 text-base text-[color:inherit] outline-none focus:border-[color:var(--mc-accent)]"
              value={String(configDraft.stt_model || "whisper-1")}
              onChange={(e) => setConfigField("stt_model", e.target.value)}
            >
              <option value="whisper-1">whisper-1</option>
              <option value="whisper-large-v3">whisper-large-v3</option>
              <option value="whisper-large-v3-turbo">
                whisper-large-v3-turbo
              </option>
            </select>
          </ConfigFieldCard>
        </div>
      </div>

      {/* Image Generation Section */}
      <div className={`${sectionCardClass} mt-4`} style={sectionCardStyle}>
        <Text size="3" weight="bold">
          Image Generation
        </Text>
        <Text size="1" color="gray" className="mt-1 block">
          Generate images from text prompts. Supported providers:{" "}
          <code>openai</code>, <code>flux</code>.
        </Text>
        <div className="mt-4 grid grid-cols-1 gap-3">
          <ConfigToggleCard
            label="image_gen_enabled"
            description={<>Enable image generation.</>}
            checked={Boolean(configDraft.image_gen_enabled)}
            onCheckedChange={(checked) =>
              setConfigField("image_gen_enabled", checked)
            }
            className={toggleCardClass}
            style={toggleCardStyle}
          />
        </div>
        <div className="mt-3 space-y-3">
          <ConfigFieldCard
            label="image_gen_provider"
            description={
              <>
                Image generation backend. <code>openai</code> uses DALL-E;{" "}
                <code>flux</code> uses the Flux API via fal.ai.
              </>
            }
          >
            <select
              className="mt-2 w-full rounded-md border border-[color:var(--mc-border-soft)] bg-transparent px-3 py-2 text-base text-[color:inherit] outline-none focus:border-[color:var(--mc-accent)]"
              value={String(configDraft.image_gen_provider || "openai")}
              onChange={(e) =>
                setConfigField("image_gen_provider", e.target.value)
              }
            >
              <option value="openai">openai (DALL-E)</option>
              <option value="flux">flux (fal.ai)</option>
            </select>
          </ConfigFieldCard>
          <ConfigFieldCard
            label="image_gen_api_key"
            description={
              <>
                API key for the image generation provider. Leave blank to keep
                current secret unchanged.
              </>
            }
          >
            <TextField.Root
              className="mt-2"
              value={String(configDraft.image_gen_api_key || "")}
              onChange={(e) =>
                setConfigField("image_gen_api_key", e.target.value)
              }
              placeholder="sk-..."
            />
          </ConfigFieldCard>
        </div>
      </div>

      {/* Video Generation Section */}
      <div className={`${sectionCardClass} mt-4`} style={sectionCardStyle}>
        <Text size="3" weight="bold">
          Video Generation
        </Text>
        <Text size="1" color="gray" className="mt-1 block">
          Generate short videos from text prompts. Supported providers:{" "}
          <code>sora</code>, <code>fal</code>, <code>minimax</code>.
        </Text>
        <div className="mt-4 grid grid-cols-1 gap-3">
          <ConfigToggleCard
            label="video_gen_enabled"
            description={<>Enable video generation.</>}
            checked={Boolean(configDraft.video_gen_enabled)}
            onCheckedChange={(checked) =>
              setConfigField("video_gen_enabled", checked)
            }
            className={toggleCardClass}
            style={toggleCardStyle}
          />
        </div>
        <div className="mt-3 space-y-3">
          <ConfigFieldCard
            label="video_gen_provider"
            description={
              <>
                Video generation backend. <code>sora</code> uses OpenAI Sora;{" "}
                <code>fal</code> uses fal.ai; <code>minimax</code> uses
                MiniMax Video.
              </>
            }
          >
            <select
              className="mt-2 w-full rounded-md border border-[color:var(--mc-border-soft)] bg-transparent px-3 py-2 text-base text-[color:inherit] outline-none focus:border-[color:var(--mc-accent)]"
              value={String(configDraft.video_gen_provider || "sora")}
              onChange={(e) =>
                setConfigField("video_gen_provider", e.target.value)
              }
            >
              <option value="sora">sora (OpenAI)</option>
              <option value="fal">fal (fal.ai)</option>
              <option value="minimax">minimax</option>
            </select>
          </ConfigFieldCard>
          <ConfigFieldCard
            label="video_gen_api_key"
            description={
              <>
                API key for the video generation provider. Leave blank to keep
                current secret unchanged.
              </>
            }
          >
            <TextField.Root
              className="mt-2"
              value={String(configDraft.video_gen_api_key || "")}
              onChange={(e) =>
                setConfigField("video_gen_api_key", e.target.value)
              }
              placeholder="sk-..."
            />
          </ConfigFieldCard>
        </div>
      </div>

      {/* Vision Fallback Section */}
      <div className={`${sectionCardClass} mt-4`} style={sectionCardStyle}>
        <Text size="3" weight="bold">
          Vision Fallback
        </Text>
        <Text size="1" color="gray" className="mt-1 block">
          Route image inputs to a vision-capable model when the primary
          provider does not support vision.
        </Text>
        <div className="mt-4 grid grid-cols-1 gap-3">
          <ConfigToggleCard
            label="vision_fallback_enabled"
            description={
              <>
                Enable vision fallback routing via OpenRouter or a
                vision-capable model.
              </>
            }
            checked={Boolean(configDraft.vision_fallback_enabled)}
            onCheckedChange={(checked) =>
              setConfigField("vision_fallback_enabled", checked)
            }
            className={toggleCardClass}
            style={toggleCardStyle}
          />
        </div>
        <div className="mt-3 space-y-3">
          <ConfigFieldCard
            label="vision_fallback_model"
            description={
              <>
                Model id to use for vision requests (e.g.{" "}
                <code>openai/gpt-4o</code> via OpenRouter).
              </>
            }
          >
            <TextField.Root
              className="mt-2"
              value={String(configDraft.vision_fallback_model || "")}
              onChange={(e) =>
                setConfigField("vision_fallback_model", e.target.value)
              }
              placeholder="openai/gpt-4o"
            />
          </ConfigFieldCard>
          <ConfigFieldCard
            label="vision_openrouter_api_key"
            description={
              <>
                OpenRouter API key for vision fallback routing. Leave blank to
                keep current secret unchanged.
              </>
            }
          >
            <TextField.Root
              className="mt-2"
              value={String(configDraft.vision_openrouter_api_key || "")}
              onChange={(e) =>
                setConfigField("vision_openrouter_api_key", e.target.value)
              }
              placeholder="sk-or-..."
            />
          </ConfigFieldCard>
        </div>
      </div>

      {/* Documents Section */}
      <div className={`${sectionCardClass} mt-4`} style={sectionCardStyle}>
        <Text size="3" weight="bold">
          Documents
        </Text>
        <Text size="1" color="gray" className="mt-1 block">
          Extract and index text from uploaded documents (PDF, DOCX, etc.)
          for use in conversations.
        </Text>
        <div className="mt-4 grid grid-cols-1 gap-3">
          <ConfigToggleCard
            label="documents_enabled"
            description={
              <>
                Enable document extraction and indexing. Uploaded files will
                be parsed and their text injected into context.
              </>
            }
            checked={Boolean(configDraft.documents_enabled)}
            onCheckedChange={(checked) =>
              setConfigField("documents_enabled", checked)
            }
            className={toggleCardClass}
            style={toggleCardStyle}
          />
        </div>
      </div>
    </>
  );
}
