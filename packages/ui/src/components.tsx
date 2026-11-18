import { createUniqueId, Show, splitProps, type JSX } from "solid-js";
import { CompactSelect } from "./CompactSelect";

export function cx(
  ...values: Array<string | false | null | undefined>
): string {
  return values.filter(Boolean).join(" ");
}

type ButtonProps = JSX.ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "secondary" | "ghost" | "danger";
  size?: "md" | "sm";
  loading?: boolean;
  block?: boolean;
};

export function Button(props: ButtonProps) {
  const [local, rest] = splitProps(props, [
    "variant",
    "size",
    "loading",
    "block",
    "class",
    "children",
    "type",
  ]);
  return (
    <button
      {...rest}
      type={local.type ?? "button"}
      class={cx(
        "btn",
        `btn--${local.variant ?? "primary"}`,
        local.size === "sm" && "btn--sm",
        local.block && "btn--block",
        local.class,
      )}
      disabled={rest.disabled || local.loading}
    >
      <Show when={local.loading}>
        <span class="spinner" aria-hidden="true" />
      </Show>
      {local.children}
    </button>
  );
}

export function Badge(props: {
  tone?: "neutral" | "accent" | "success" | "warning" | "danger";
  children: JSX.Element;
}) {
  return (
    <span class={`badge badge--${props.tone ?? "neutral"}`}>
      {props.children}
    </span>
  );
}

type FieldProps = Omit<JSX.InputHTMLAttributes<HTMLInputElement>, "onInput"> & {
  label: string;
  hint?: string;
  error?: string;
  onInput?: (value: string) => void;
};

export function Field(props: FieldProps) {
  const [local, rest] = splitProps(props, [
    "label",
    "hint",
    "error",
    "onInput",
    "class",
    "id",
  ]);
  const id = local.id ?? createUniqueId();
  return (
    <div class={cx("field", local.class)}>
      <label class="field__label" for={id}>
        {local.label}
      </label>
      <input
        {...rest}
        id={id}
        class="input"
        aria-invalid={local.error ? "true" : undefined}
        onInput={(event) => local.onInput?.(event.currentTarget.value)}
      />
      <Show when={local.error}>
        <span class="field__error" role="alert">
          {local.error}
        </span>
      </Show>
      <Show when={local.hint && !local.error}>
        <span class="field__hint">{local.hint}</span>
      </Show>
    </div>
  );
}

type SelectProps = {
  label?: string;
  value: string;
  options: Array<{ value: string; label: string }>;
  onChange: (value: string) => void;
  id?: string;
  disabled?: boolean;
  class?: string;
  ariaLabel?: string;
};

export function SelectField(props: SelectProps) {
  const id = props.id ?? createUniqueId();
  return (
    <div class={cx("field", props.class)}>
      <Show when={props.label}>
        <label class="field__label" for={id}>
          {props.label}
        </label>
      </Show>
      <CompactSelect
        id={id}
        ariaLabel={props.ariaLabel ?? props.label ?? "Select"}
        value={props.value}
        disabled={props.disabled}
        onChange={props.onChange}
        options={props.options}
      />
    </div>
  );
}

type TextareaProps = {
  label: string;
  value: string;
  onInput: (value: string) => void;
  id?: string;
  placeholder?: string;
  hint?: string;
  rows?: number;
  disabled?: boolean;
};

export function TextareaField(props: TextareaProps) {
  const id = props.id ?? createUniqueId();
  return (
    <div class="field">
      <label class="field__label" for={id}>
        {props.label}
      </label>
      <textarea
        id={id}
        class="textarea"
        rows={props.rows ?? 4}
        placeholder={props.placeholder}
        disabled={props.disabled}
        value={props.value}
        onInput={(event) => props.onInput(event.currentTarget.value)}
      />
      <Show when={props.hint}>
        <span class="field__hint">{props.hint}</span>
      </Show>
    </div>
  );
}

export function EmptyState(props: {
  title: string;
  description: string;
  icon?: JSX.Element;
  action?: JSX.Element;
}) {
  return (
    <div class="empty">
      <Show when={props.icon}>
        <div class="empty__icon">{props.icon}</div>
      </Show>
      <h3 class="empty__title">{props.title}</h3>
      <p class="empty__text">{props.description}</p>
      {props.action}
    </div>
  );
}

export function Notice(props: {
  tone?: "neutral" | "error" | "success";
  children: JSX.Element;
}) {
  return (
    <div
      class={cx("notice", props.tone && `notice--${props.tone}`)}
      role="status"
    >
      {props.children}
    </div>
  );
}

export function StatusChip(props: {
  kind: "idle" | "loading" | "saving" | "saved" | "error";
  message?: string;
}) {
  const label = () =>
    props.message ??
    {
      idle: "Local and ready",
      loading: "Loading…",
      saving: "Saving…",
      saved: "Saved locally",
      error: "Error",
    }[props.kind];
  return (
    <span
      class={cx("status-chip", `status-chip--${props.kind}`)}
      aria-live="polite"
    >
      <Show when={props.kind === "saving" || props.kind === "loading"}>
        <span class="spinner" aria-hidden="true" />
      </Show>
      {label()}
    </span>
  );
}
