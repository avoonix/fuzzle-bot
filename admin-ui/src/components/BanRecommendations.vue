<script setup lang="ts">
import { useFetch } from '@vueuse/core';
import { computed, ref, watch } from 'vue';
import BanStickersGroup from './BanStickersGroup.vue';

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

    <!-- <v-btn @click="toggleBan" v-if="isBanned" color="error">
      Unban
    </v-btn>
    <v-btn @click="toggleBan" v-else color="success">
      Ban
    </v-btn> -->

    <div v-if="data">
      <v-card v-for="rec of data" class="mb-8 mx-2">
        <v-card-text>
          banned:
          <img :src="`/api/banned-sticker/${rec.banned_sticker.id}/thumbnail.png`" loading="lazy" width="128"
            height="128" />
          recommended:

                    <ban-stickers-group :refetch="() => console.log('refetch not implemented')" :stickers="rec.matches" />
        </v-card-text>
      </v-card>
    </div>
  </div>
</template>
