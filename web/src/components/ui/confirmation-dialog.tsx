import { type ReactNode, useState } from "react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogClose, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";

export interface ConfirmationDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description: ReactNode;
  confirmText: string;
  cancelText?: string;
  tone?: "default" | "destructive";
  /** What the confirm button does. The dialog stays open while it runs, and closes when it is done. */
  onConfirm: () => void | Promise<void>;
}

/** Asks before something that cannot be taken back, and does it when it is confirmed. */
export function ConfirmationDialog({
  open,
  onOpenChange,
  title,
  description,
  confirmText,
  cancelText = "Cancel",
  tone = "default",
  onConfirm,
}: ConfirmationDialogProps) {
  const [confirming, setConfirming] = useState(false);

  const confirm = async () => {
    setConfirming(true);
    try {
      await onConfirm();
      onOpenChange(false);
    } finally {
      setConfirming(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent showCloseButton={false}>
        <DialogHeader showSeparator={false}>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>{description}</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <DialogClose render={<Button variant="outline" disabled={confirming} />}>{cancelText}</DialogClose>
          <Button variant={tone} disabled={confirming} onClick={confirm}>
            {confirmText}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
