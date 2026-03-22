import { CommandRef } from "./components/command-ref";
import { Footer } from "./components/footer";
import { Hero } from "./components/hero";
import { Install } from "./components/install";
import cliData from "./data/cli.json";

export function App() {
  return (
    <div className="min-h-screen bg-neutral-950 text-neutral-100">
      <main className="mx-auto max-w-2xl px-6 py-24">
        <Hero />
        <Install />
        <CommandRef data={cliData} />
        <Footer />
      </main>
    </div>
  );
}
