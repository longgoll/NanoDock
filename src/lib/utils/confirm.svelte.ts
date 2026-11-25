export interface ConfirmOptions {
  title: string;
  message: string;
  label?: string;
  danger?: boolean;
  action: () => Promise<void>;
}

/**
 * Creates reactive state and handlers for ConfirmModal across resource views.
 */
export function useConfirm() {
  let open = $state(false);
  let title = $state("");
  let message = $state("");
  let confirmLabel = $state("Confirm");
  let danger = $state(true);
  let pendingAction = $state<(() => Promise<void>) | null>(null);

  function request(opts: ConfirmOptions) {
    title = opts.title;
    message = opts.message;
    confirmLabel = opts.label ?? "Delete";
    danger = opts.danger ?? true;
    pendingAction = opts.action;
    open = true;
  }

  async function execute() {
    if (!pendingAction) return;
    const fn = pendingAction;
    open = false;
    pendingAction = null;
    await fn();
  }

  function cancel() {
    open = false;
    pendingAction = null;
  }

  return {
    get open() {
      return open;
    },
    get title() {
      return title;
    },
    get message() {
      return message;
    },
    get confirmLabel() {
      return confirmLabel;
    },
    get danger() {
      return danger;
    },
    request,
    execute,
    cancel,
  };
}
