<script setup lang="ts">
import { useFetch } from '@vueuse/core';
import { computed, ref, watch } from 'vue';
import BanStickersGroup from './BanStickersGroup.vue';
import BanSetStickersDialog from './BanSetStickersDialog.vue';

interface BanRecommendation {
  matches: StickerSimPub[],
  banned_sticker: StickerPub,
}

interface StickerPub {
  id: string,
  set_id: string,
}

interface StickerSimPub extends StickerPub {
  similarity: number
}

const { data, error, execute: refetch } = useFetch(`/api/recommend-stickers-for-ban`, { refetch: true, updateDataOnError: true }).json<BanRecommendation[]>()

watch(error, () => console.log(error))

const setMode = ref(false);

const setData = computed(() => {
  const sets: Record<string, number> = {};
  for (const entry of data.value??[]) {
    for (const sticker of entry.matches) {
      sets[sticker.set_id] = (sets[sticker.set_id] ?? 0) + 1;
    }
  }
  return Object.entries(sets).map(([setId, count]) => ({setId, count})).toSorted((a,b) => b.count - a.count);
})

// const maxSimilarity = ref(0.9)

// const filteredSticker = computed(() => (data.value || []).filter(s => s.similarity >= maxSimilarity.value))

// watch(url, () => isBanned.value = false);

// const isBanned = ref(false)

// const toggleBan = async () => {
//   if (isBanned.value) {
//     const { data, error } = await useFetch(`/api/stickers/${route.params.stickerId}/unban`).post()
//     console.log(data, error)
//     if (!error.value) {
//       isBanned.value = false;
//     }
//   } else {
//     const { data, error } = await useFetch(`/api/stickers/${route.params.stickerId}/ban`).post({
//       clip_max_match_distance: maxSimilarity.value
//     })
//     console.log(data, error)
//     if (!error.value) {
//       isBanned.value = true;
//     }
//   }
// }

const unbanSticker = async (stickerId: string) => {
    const { data, error } = await useFetch(`/api/stickers/${stickerId}/unban`).post()
    console.log(data, error)
    if (error.value) {
      alert(error.value)
    }
}

</script>

<template>
  <div class="asdf">
    <h1>Ban Recommendations</h1>
    <!-- <v-slider
        v-model="maxSimilarity"
        thumb-label
        min="0.6"
        max="1"
        step="0.005"
      ></v-slider> -->
    {{ error }}
    
    <v-switch v-model="setMode" label="Set Mode" />
    
      <div v-if="data && setMode">

      <v-card v-for="set of setData" class="mb-8 mx-2">
            <!-- <v-btn variant="plain" width="auto" height="auto" :to="{ name: 'banSetView', params: { setId: set.setid } }"> -->
                <img :src="`/thumbnails/sticker-set/${set.setId}/image.png`" loading="lazy" width="128" height="128" />
                {{ set }}
              <!-- </v-btn> -->
        <ban-set-stickers-dialog :set-id="set.setId" />
      </v-card>

      </div>

    <!-- <v-btn @click="toggleBan" v-if="isBanned" color="error">
      Unban
    </v-btn>
    <v-btn @click="toggleBan" v-else color="success">
      Ban
    </v-btn> -->

    <div v-if="data && !setMode">
      <v-card v-for="rec of data" class="mb-8 mx-2">
        <v-card-text>
          banned:
          <img :src="`/api/banned-sticker/${rec.banned_sticker.id}/thumbnail.png`" loading="lazy" width="128"
            height="128" />

          <v-btn @click="unbanSticker(rec.banned_sticker.id)">
            unban sticker
          </v-btn>
          recommended:

                    <ban-stickers-group :refetch="() => console.log('refetch not implemented')" :stickers="rec.matches" />
        </v-card-text>
      </v-card>
    </div>
  </div>
</template>
