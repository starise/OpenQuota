<script lang="ts">
  import { flushSync, onMount } from 'svelte';

  let bubble: HTMLDivElement;
  let text = $state('');
  let anchor: HTMLElement | null = null;
  let showTimer: ReturnType<typeof setTimeout> | undefined;

  function findAnchor(target: EventTarget | null) {
    const element = target instanceof Element ? target.closest('[data-tooltip]') : null;
    return element instanceof HTMLElement && element.dataset.tooltip ? element : null;
  }

  function hide() {
    clearTimeout(showTimer);
    anchor = null;
    bubble.hidden = true;
  }

  function show() {
    const label = anchor?.dataset.tooltip;
    if (!anchor?.isConnected || !label) {
      hide();
      return;
    }

    flushSync(() => (text = label));
    bubble.hidden = false;
    const trigger = anchor.getBoundingClientRect();
    const { width, height } = bubble.getBoundingClientRect();
    const align = anchor.dataset.tooltipAlign;
    const left =
      align === 'left'
        ? trigger.left
        : align === 'right'
          ? trigger.right - width
          : trigger.left + (trigger.width - width) / 2;
    const below = trigger.bottom + 7;
    const above = trigger.top - height - 7;
    const top =
      above >= 8 && (anchor.dataset.tooltipPlacement === 'top' || below + height + 8 > innerHeight)
        ? above
        : below;

    bubble.style.left = `${Math.max(8, Math.min(left, innerWidth - width - 8))}px`;
    bubble.style.top = `${Math.max(8, Math.min(top, innerHeight - height - 8))}px`;
  }

  function schedule(next: HTMLElement | null) {
    if (next === anchor) return;
    hide();
    if (!next) return;
    anchor = next;
    showTimer = setTimeout(show, 400);
  }

  function handlePointer(event: PointerEvent) {
    if (event.pointerType === 'touch') return;
    const target = event.type === 'pointerout' ? event.relatedTarget : event.target;
    const focused = document.activeElement;
    schedule(
      findAnchor(target) ?? (focused?.matches(':focus-visible') ? findAnchor(focused) : null),
    );
  }

  function handleFocus(event: FocusEvent) {
    if (event.type === 'focusout') {
      if (anchor && anchor === event.target && !anchor.matches(':hover')) hide();
    } else if (event.target instanceof Element && event.target.matches(':focus-visible')) {
      schedule(findAnchor(event.target));
    }
  }

  onMount(() => {
    const observer = new MutationObserver(() => {
      if (!anchor) return;
      if (!anchor.isConnected || !anchor.dataset.tooltip) hide();
      else if (!bubble.hidden && anchor.dataset.tooltip !== text) show();
    });
    observer.observe(document.body, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ['data-tooltip'],
    });
    return () => {
      observer.disconnect();
      clearTimeout(showTimer);
    };
  });
</script>

<svelte:window
  onpointerover={handlePointer}
  onpointerout={handlePointer}
  onfocusin={handleFocus}
  onfocusout={handleFocus}
  onscrollcapture={hide}
  onresize={hide}
  onblur={hide}
  onkeydowncapture={(event) => {
    if (event.key === 'Escape') hide();
  }}
/>

<div class="tooltip" role="tooltip" bind:this={bubble} hidden>{text}</div>

<style>
  .tooltip {
    position: fixed;
    z-index: 50;
    width: max-content;
    max-width: min(206px, calc(100vw - 24px));
    padding: 5px 7px;
    border: 1px solid var(--separator);
    border-radius: 6px;
    color: var(--text);
    background: color-mix(in srgb, var(--tray) 96%, transparent);
    box-shadow: 0 5px 18px rgba(0, 0, 0, 0.24);
    font-size: 10px;
    font-weight: 400;
    line-height: 13px;
    white-space: pre-line;
    overflow-wrap: anywhere;
    pointer-events: none;
  }
</style>
