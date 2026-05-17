import { MetaFunction } from "@remix-run/node";

export const meta: MetaFunction = () => [
  { title: "About | Bunge Bits" },
  { name: "description", content: "Learn more about the Bunge Bits project" },
];

export default function AboutPage() {
  return (
    <div className="max-w-3xl mx-auto px-8 py-12 space-y-6">
      <h1 className="text-3xl font-bold">About Bunge Bits</h1>

      <p>
        <strong>Bunge Bits</strong> is a civic tech project that uses AI to generate
        plain-language summaries of Kenya’s National Assembly and Senate proceedings,
        making parliamentary activity more accessible to all Kenyans.
      </p>

      <h2 className="text-xl font-semibold mt-6">Where does the data come from?</h2>
      <p>
        Bunge Bits processes archived livestreams from the official Parliament of Kenya
        YouTube channel. Audio is transcribed and summarised automatically, producing
        structured highlights of each sitting.
      </p>

      <h2 className="text-xl font-semibold mt-6">Open source</h2>
      <p>
        Bunge Bits is open source. The code is available on{" "}
        <a
          href="https://github.com/mwananchi-tech/bunge-bits"
          target="_blank"
          rel="noopener noreferrer"
          className="underline hover:text-primary"
        >
          GitHub
        </a>
        . Built by{" "}
        <a
          href="https://c12i.xyz"
          target="_blank"
          rel="noopener noreferrer"
          className="underline"
        >
          Collins Muriuki
        </a>{" "}
        under{" "}
        <a
          href="https://mwananchi-tech.github.io"
          target="_blank"
          rel="noopener noreferrer"
          className="underline"
        >
          Mwananchi Tech
        </a>
        .
      </p>

      <div className="rounded-md border px-5 py-4 mt-6 space-y-2">
        <p className="font-semibold">Bunge Bits is being succeeded by Bunge Hub.</p>
        <p className="text-sm text-muted-foreground">
          Bunge Hub is a new platform built on Kenya’s official Hansard rather than
          AI-generated transcripts, making it more accurate, sustainable, and
          comprehensive. It covers every bill, debate, and contribution from the 13th
          Parliament.
        </p>
        <a
          href="https://collinsmuriuki.xyz/from-bunge-bits-to-bunge-hub/"
          target="_blank"
          rel="noopener noreferrer"
          className="text-sm underline hover:text-primary inline-block"
        >
          Read the full story →
        </a>
      </div>
    </div>
  );
}
