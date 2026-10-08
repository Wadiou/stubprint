import { Card, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

export function Placeholder() {
  return (
    <Card className="max-w-lg">
      <CardHeader>
        <CardTitle>Not built yet</CardTitle>
        <CardDescription>This screen comes in a later step.</CardDescription>
      </CardHeader>
    </Card>
  );
}
