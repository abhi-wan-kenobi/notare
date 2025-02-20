<script setup>
    const cards = [
        {
            title: "Create Notes",
            url: "/extensions/notion",
            body: "Start writing and organizing your thoughts instantly with our markdown editor."
        },
        {
            title: "Sync Anywhere",
            url: "/extensions/notion",
            body: "Your notes are stored locally first, with optional cloud sync when you need it."
        },
        {
            title: "Stay Organized",
            url: "/extensions/notion",
            body: "Use tags, folders, and powerful search to keep your notes organized."
        },
        {
            title: "Work Offline",
            url: "/extensions/notion",
            body: "Access and edit your notes even without an internet connection."
        }
    ]
</script>

<h1 class="flex items-center gap-2"><div class="i-heroicons-bolt-20-solid h-8 w-8"></div> Quickstart</h1>

Since `Hyprnote` is built with `local-first` in mind, **no signup is required** to get started.

<div class="grid grid-cols-2 gap-4 my-8">
  <Card v-for="card in cards" :key="card.title" :title="card.title" :url="card.url" :body="card.body"/>
</div>

