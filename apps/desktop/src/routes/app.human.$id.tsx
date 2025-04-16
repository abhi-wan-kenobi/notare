import { zodResolver } from "@hookform/resolvers/zod";
import { Trans } from "@lingui/react/macro";
import { RiCornerDownLeftLine } from "@remixicon/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, notFound, useNavigate, useRouter } from "@tanstack/react-router";
import { BuildingIcon, CircleMinus, SearchIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";

import { EditableEntityWrapper } from "@/components/toolbar/bars";
import { useEditMode } from "@/contexts";
import { commands as dbCommands, type Human, type Organization } from "@hypr/plugin-db";
import { Form, FormControl, FormField, FormItem, FormMessage } from "@hypr/ui/components/ui/form";
import { Input } from "@hypr/ui/components/ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "@hypr/ui/components/ui/popover";

export const Route = createFileRoute("/app/human/$id")({
  component: Component,
  loader: async ({ context: { queryClient }, params }: { context: { queryClient: any }; params: { id: string } }) => {
    const human: Human | null = await queryClient.fetchQuery({
      queryKey: ["human", params.id],
      queryFn: () => dbCommands.getHuman(params.id),
    });

    if (!human) {
      throw notFound();
    }

    if (!human.organization_id) {
      return { human, organization: null };
    }

    const organization = await queryClient.fetchQuery({
      queryKey: ["org", human.organization_id],
      queryFn: () => dbCommands.getOrganization(human.organization_id!),
    });

    return { human, organization };
  },
});

const formSchema = z.object({
  full_name: z.string().optional(),
  job_title: z.string().optional(),
  email: z.string().email().optional(),
  linkedin_username: z.string().optional(),
  organization_id: z.string().nullable(),
});

type FormSchema = z.infer<typeof formSchema>;

function Component() {
  const { human, organization } = Route.useLoaderData();
  const router = useRouter();
  const { isEditing } = useEditMode();

  const form = useForm<FormSchema>({
    resolver: zodResolver(formSchema),
    values: {
      full_name: human.full_name ?? "",
      job_title: human.job_title ?? "",
      email: human.email ?? "",
      linkedin_username: human.linkedin_username ?? "",
      organization_id: human.organization_id,
    },
  });

  const updateHumanMutation = useMutation({
    mutationFn: (data: Partial<Human>) =>
      dbCommands.upsertHuman({
        ...human,
        ...data,
      }),
    onSuccess: () => {
      router.invalidate();
    },
  });

  useEffect(() => {
    if (!isEditing) {
      form.handleSubmit((v) => updateHumanMutation.mutate(v))();
    }
  }, [isEditing]);

  return (
    <EditableEntityWrapper>
      {isEditing ? <HumanEdit form={form} /> : <HumanView value={human} organization={organization} />}
    </EditableEntityWrapper>
  );
}

function HumanView({ value, organization }: { value: Human; organization: Organization | null }) {
  const navigate = useNavigate();

  const handleClickOrganization = () => {
    if (organization) {
      navigate({ to: "/app/organization/$id", params: { id: organization.id } });
    }
  };

  return (
    <div className="space-y-6 p-4 max-w-2xl">
      <div className="pb-4 border-b border-neutral-200">
        <h1 className="text-2xl font-semibold text-neutral-800">{value.full_name || "Unnamed Contact"}</h1>
        {value.job_title && <p className="text-neutral-500 mt-1">{value.job_title}</p>}
        {organization && (
          <div
            className="flex items-center mt-2 text-sm text-neutral-600 hover:underline cursor-pointer"
            onClick={handleClickOrganization}
          >
            <BuildingIcon className="mr-1.5 size-4 text-neutral-400" />
            {organization.name}
          </div>
        )}
      </div>

      <div className="space-y-4">
        {value.email && (
          <div className="flex flex-col">
            <span className="text-xs text-neutral-500 uppercase tracking-wider mb-1">Email</span>
            <span className="text-neutral-800">{value.email}</span>
          </div>
        )}

        {value.job_title && (
          <div className="flex flex-col">
            <span className="text-xs text-neutral-500 uppercase tracking-wider mb-1">Job Title</span>
            <span className="text-neutral-800">{value.job_title}</span>
          </div>
        )}

        {value.linkedin_username && (
          <div className="flex flex-col">
            <span className="text-xs text-neutral-500 uppercase tracking-wider mb-1">LinkedIn</span>
            <a
              href={`https://linkedin.com/in/${value.linkedin_username}`}
              target="_blank"
              rel="noopener noreferrer"
              className="text-blue-600 hover:underline"
            >
              {value.linkedin_username}
            </a>
          </div>
        )}
      </div>
    </div>
  );
}

function HumanEdit({ form }: { form: ReturnType<typeof useForm<FormSchema>> }) {
  return (
    <div>
      <Form {...form}>
        <form className="space-y-8">
          <FormField
            control={form.control}
            name="full_name"
            render={({ field }) => (
              <FormItem>
                <FormControl>
                  <Input placeholder="Full Name" {...field} />
                </FormControl>
                <FormMessage />
              </FormItem>
            )}
          />
          <FormField
            control={form.control}
            name="email"
            render={({ field }) => (
              <FormItem>
                <FormControl>
                  <Input type="email" placeholder="Email" {...field} />
                </FormControl>
                <FormMessage />
              </FormItem>
            )}
          />
          <FormField
            control={form.control}
            name="job_title"
            render={({ field }) => (
              <FormItem>
                <FormControl>
                  <Input placeholder="Job Title" {...field} />
                </FormControl>
                <FormMessage />
              </FormItem>
            )}
          />
          <FormField
            control={form.control}
            name="linkedin_username"
            render={({ field }) => (
              <FormItem>
                <FormControl>
                  <Input placeholder="LinkedIn Username" {...field} />
                </FormControl>
                <FormMessage />
              </FormItem>
            )}
          />

          <div className="space-y-2">
            <h3>Organization</h3>
            <OrganizationSelector form={form} />
          </div>
        </form>
      </Form>
    </div>
  );
}

function OrganizationSelector({ form }: { form: ReturnType<typeof useForm<FormSchema>> }) {
  const orgId = form.watch("organization_id");
  const [open, setOpen] = useState(false);

  const { data: organization } = useQuery({
    queryKey: ["org", orgId],
    queryFn: () => orgId ? dbCommands.getOrganization(orgId) : null,
    enabled: !!orgId,
  });

  const handleRemoveOrganization = () => {
    form.setValue("organization_id", null);
  };

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <div className="flex flex-row items-center gap-1 rounded-md px-2 py-1.5 hover:bg-neutral-100 text-xs border border-neutral-200 cursor-pointer w-fit">
          <BuildingIcon size={14} />
          {organization
            ? (
              <div className="flex items-center">
                <span>{organization.name}</span>
                <span className="ml-2 text-neutral-400 group">
                  <CircleMinus
                    className="size-4 cursor-pointer text-neutral-400 hover:text-red-600"
                    onClick={(e) => {
                      e.stopPropagation();
                      handleRemoveOrganization();
                    }}
                  />
                </span>
              </div>
            )
            : <span>Select organization</span>}
        </div>
      </PopoverTrigger>

      <PopoverContent className="shadow-lg p-3" align="start" side="bottom">
        <OrganizationControl form={form} closePopover={() => setOpen(false)} />
      </PopoverContent>
    </Popover>
  );
}

function OrganizationControl(
  { form, closePopover }: { form: ReturnType<typeof useForm<FormSchema>>; closePopover: () => void },
) {
  const queryClient = useQueryClient();
  const [searchTerm, setSearchTerm] = useState("");

  const addOrganizationMutation = useMutation({
    mutationFn: async ({ name }: { name: string }) => {
      const newOrg = await dbCommands.upsertOrganization({
        id: crypto.randomUUID(),
        name,
        description: null,
      });

      form.setValue("organization_id", newOrg.id);
      return newOrg;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: ["organizations"],
      });
      closePopover();
    },
  });

  const { data: organizations = [] } = useQuery({
    queryKey: ["organizations", searchTerm],
    queryFn: () => {
      if (!searchTerm) {
        return dbCommands.listOrganizations(null);
      }
      return dbCommands.listOrganizations({ search: [5, searchTerm] });
    },
  });

  const handleSubmit = (e: React.SyntheticEvent<HTMLFormElement>) => {
    e.preventDefault();

    const name = searchTerm.trim();
    if (name === "") {
      return;
    }

    addOrganizationMutation.mutate({ name });
    setSearchTerm("");
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();

      const name = searchTerm.trim();
      if (name === "") {
        return;
      }

      addOrganizationMutation.mutate({ name });
      setSearchTerm("");
    }
  };

  const selectOrganization = (orgId: string) => {
    form.setValue("organization_id", orgId);
    closePopover();
  };

  return (
    <div className="flex flex-col gap-3 max-w-[450px]">
      <div className="text-sm font-medium text-neutral-700">Organization</div>

      <form onSubmit={handleSubmit}>
        <div className="flex flex-col gap-2">
          <div className="flex items-center w-full px-2 py-1.5 gap-2 rounded bg-neutral-50 border border-neutral-200">
            <span className="text-neutral-500 flex-shrink-0">
              <SearchIcon className="size-4" />
            </span>
            <input
              type="text"
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Search or add organization"
              className="w-full bg-transparent text-sm focus:outline-none placeholder:text-neutral-400"
            />
            {searchTerm.trim() && (
              <button
                type="submit"
                className="text-neutral-500 hover:text-neutral-700 transition-colors flex-shrink-0"
                aria-label="Add organization"
              >
                <RiCornerDownLeftLine className="size-4" />
              </button>
            )}
          </div>

          {searchTerm.trim() && (
            <div className="flex flex-col w-full rounded border border-neutral-200 overflow-hidden">
              {organizations.map((org) => (
                <button
                  key={org.id}
                  type="button"
                  className="flex items-center px-3 py-2 text-sm text-left hover:bg-neutral-100 transition-colors w-full"
                  onClick={() => selectOrganization(org.id)}
                >
                  <span className="flex-shrink-0 size-5 flex items-center justify-center mr-2 bg-neutral-100 rounded-full">
                    <BuildingIcon className="size-3" />
                  </span>
                  <span className="font-medium truncate">{org.name}</span>
                </button>
              ))}

              {organizations.length === 0 && (
                <button
                  type="button"
                  className="flex items-center px-3 py-2 text-sm text-left hover:bg-neutral-100 transition-colors w-full"
                  onClick={() => addOrganizationMutation.mutate({ name: searchTerm.trim() })}
                >
                  <span className="flex-shrink-0 size-5 flex items-center justify-center mr-2 bg-neutral-200 rounded-full">
                    <span className="text-xs">+</span>
                  </span>
                  <span className="flex items-center gap-1 font-medium text-neutral-600">
                    <Trans>Create</Trans>
                    <span className="text-neutral-900 truncate max-w-[140px]">&quot;{searchTerm.trim()}&quot;</span>
                  </span>
                </button>
              )}
            </div>
          )}

          {!searchTerm.trim() && organizations.length > 0 && (
            <div className="flex flex-col w-full rounded border border-neutral-200 overflow-hidden max-h-[40vh] overflow-y-auto custom-scrollbar">
              {organizations.map((org) => (
                <button
                  key={org.id}
                  type="button"
                  className="flex items-center px-3 py-2 text-sm text-left hover:bg-neutral-100 transition-colors w-full"
                  onClick={() => selectOrganization(org.id)}
                >
                  <span className="flex-shrink-0 size-5 flex items-center justify-center mr-2 bg-neutral-100 rounded-full">
                    <BuildingIcon className="size-3" />
                  </span>
                  <span className="font-medium truncate">{org.name}</span>
                </button>
              ))}
            </div>
          )}
        </div>
      </form>
    </div>
  );
}
