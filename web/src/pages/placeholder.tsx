import { Card, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

export function Placeholder({ title }: { title: string }) {
  return (
    <Card className="max-w-lg">
      <CardHeader>
        <CardTitle>{title}</CardTitle>
        <CardDescription>Not built yet.</CardDescription>
      </CardHeader>
    </Card>
  );
}
