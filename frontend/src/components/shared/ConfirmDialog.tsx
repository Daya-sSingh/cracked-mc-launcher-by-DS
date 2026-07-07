import { Modal, ModalFooter } from "./Modal";
import { Button, type ButtonVariant } from "./Button";

interface ConfirmDialogProps {
  open: boolean;
  onClose: () => void;
  onConfirm: () => void;
  title: string;
  description: string;
  confirmLabel?: string;
  /** Uses the danger button styling and red confirm action — for destructive, irreversible actions like deleting an instance. */
  isDangerous?: boolean;
  isLoading?: boolean;
}

/**
 * A themed confirmation dialog for actions that shouldn't happen from a
 * single accidental click — deleting an instance, discarding unsaved
 * changes, and similar. Built on the shared `Modal` component rather than
 * the browser's native `confirm()`, which would look jarringly out of
 * place in a custom-styled dark app.
 */
export function ConfirmDialog({
  open,
  onClose,
  onConfirm,
  title,
  description,
  confirmLabel = "Confirm",
  isDangerous = false,
  isLoading = false,
}: ConfirmDialogProps) {
  const confirmVariant: ButtonVariant = isDangerous ? "danger" : "primary";

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={title}
      description={description}
      maxWidth="max-w-sm"
      persistent={isLoading}
    >
      <ModalFooter className="mt-0 pt-0 border-t-0">
        <Button variant="ghost" onClick={onClose} disabled={isLoading}>
          Cancel
        </Button>
        <Button variant={confirmVariant} onClick={onConfirm} isLoading={isLoading}>
          {confirmLabel}
        </Button>
      </ModalFooter>
    </Modal>
  );
}
